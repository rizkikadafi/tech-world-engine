# Integrasi Godot — Rencana Implementasi

Dokumen ini menjelaskan **bagaimana engine Tech World akan diintegrasikan ke Godot**,
dari kondisi implementasi saat ini. Isinya mencakup: arsitektur target, perubahan
yang harus disiapkan pada kode existing, format dunia (`.twscene`), permukaan API
yang diekspos ke Godot, rencana verifikasi, serta risikonya.

Dokumen ini adalah **rencana implementasi**, bukan catatan hasil. Untuk melihat
apa yang benar-benar ada sekarang, baca `implementation.md`.

> Status: **rencana**. Fase A (lapisan netral) disepakati untuk dikerjakan lebih
> dulu; Fase B (adapter GDExtension) menyusul. Lihat §15 untuk keputusan yang
> masih terbuka.

---

## Ringkasan (TL;DR)

- Godot masuk sebagai **client**, bukan sebagai tempat engine hidup. Seluruh
  `Time / State / Event / Solver / Circuit / Transistor / CPU` tetap milik
  standalone engine.
- Bentuk integrasi: **GDExtension in-process** (Rust crate `godot`), bukan
  subprocess/IPC.
- Sumber kebenaran dunia: **file `.twscene` (JSON)** yang di-load engine, bukan
  scene Godot dan bukan GDScript.
- Hierarki: ada **lapisan netral baru** (`tw-scene`) di antara `tw-sim` dan
  Godot, supaya seluruh fitur bisa diuji **tanpa Godot terpasang**.
- Fase A murni netral: skema `.twscene` + registry komponen + façade client + CLI
  `--world`. Fase B: crate `tw-godot` (cdylib) + project Godot.

---

## 1. Posisi Godot dalam roadmap

Godot sudah punya tempat di roadmap, dan tempat itu **bertentangan** dengan
Phase 6 (fondasi numerik) yang sedang didekati sekarang. Roadmap yang ada:

```text
docs/engine.md          Phase 12 — External Interface / Godot
docs/engine-v1.md       Phase 16 — External API / Godot
```

Kutipan dari `engine-v1.md` (Phase 16):

> **Baru di sini Godot masuk.**
>
> Godot hanya melakukan: `visualize()` `inspect()` `pause()` `resume()` `trace()`
> `refine()` `select_fidelity()`
>
> Sedangkan: `Time` `State` `Event` `Solver` `Circuit` `Transistor` `NAND` `CPU`
> tetap milik standalone engine.

Prinsip yang diambil dari sini, dan yang mengikat seluruh dokumen ini:

1. **Arah dependensi satu-way.** `tw-core` dan `tw-sim` tidak boleh pernah
   mengimpor `godot`.
2. **Engine harus utuh tanpa Godot.** Jika `tw-sim` bisa dijalankan penuh dari
   CLI tanpa Godot, maka engine tidak "dipotong" oleh integrasi.
3. **Godot adalah operasi client**, bukan pemilik semantik simulasi.

Dokumen ini **tidak** membatalkan Phase 12/16. Yang diawasi: jalur integrasi
dibangun sekarang memakai lapisan yang sama persis dengan yang akan dipakai
nanti, sehingga tidak ada penulisan ulang besar saat Phase 12 tiba.

---

## 2. Kondisi implementasi saat ini

Fakta terverifikasi per repository:

| Area | Implementasi sekarang | File |
| --- | --- | --- |
| Workspace | `tw-core` (primitif) → `tw-sim` (loop) → `tw-cli` | `Cargo.toml` |
| Komponen | `trait Component`: `handle_event`, `state`, `ports`, `on_mount`, `on_unmount`; disimpan sebagai `Box<dyn Component>` (bukan `Send`) | `crates/tw-core/src/component/mod.rs` |
| Event | `EventPayload` enum **tertutup**; ada `PortSignal { port_id, value }` dan `Custom(String)` | `crates/tw-core/src/event/mod.rs` |
| State | `State` = `HashMap<String, StateValue>`; `StateValue` = `Bool / Int / Float / Text` | `crates/tw-core/src/state/mod.rs` |
| Waktu | `SimulationTime { ticks: u64 }` — fixed-point, deterministik | `crates/tw-core/src/time/simulation_time.rs` |
| Observasi | `trait Observer { on_event, on_snapshot }` | `crates/tw-core/src/observation/observer.rs` |
| Tracing | `CausalGraph::explain(TraceId) -> Vec<&TraceNode>`; `TraceNode` **tidak** menyimpan `ComponentId` | `crates/tw-core/src/tracing/causal_graph.rs` |
| Resource | `Resource` + `Amount{Discrete,Continuous}` + `AllocationPolicy` + `service_time` | `crates/tw-core/src/resource/mod.rs` |
| Loop | `Simulation::step()` (satu event), `run_until(tick)`, `checkpoint()`, `restore_resources()` | `crates/tw-sim/src/simulation.rs` |
| Wiring dunia | **Hardcode di Rust** sebagai skenario utuh | `crates/tw-sim/src/scenarios/*.rs` |
| Serialisasi | **Belum ada** dependensi `serde` di `tw-core`/`tw-sim` | — |
| Godot | **Nol jejak**: tidak ada dependensi `godot`, tidak ada `cdylib`, tidak ada `.gdextension` | — |

Baseline: `cargo test --workspace` → 47 test hijau.

---

## 3. Prinsip yang dijaga (invarian)

```text
INVARIAN-1   tw-core / tw-sim tidak pernah mengimpor `godot`.
INVARIAN-2   Semua perilaku simulasi Deterministik; yang non-deterministik
              hanya pemetaan wall-clock → tick.
INVARIAN-3   Godot hanya membaca state dan memberi stimulus; Godot tidak
              pernah menentukan urutan eksekusi event.
INVARIAN-4   Tidak ada f64 di dalam core. Konversi waktu terjadi di tepi.
INVARIAN-5   Godot dapat di uninstall; engine tetap bisa dijalankan via CLI.
```

INVARIAN-2 dan INVARIAN-4 adalah alasan `SimulationTime` memakai `u64 ticks` dan
bukan `f64`. Integrasi Godot tidak boleh membuka celah ini.

---

## 4. Arsitektur target

```text
Godot 4.7 (GDScript / editor plugin)
        │  Node `SimulationHost`, signals, Inspector
        ▼
tw-godot        (cdylib GDExtension)
        │  Variant ↔ StateValue, tick accumulator, GDScript callback
        ▼
tw-scene        (netral: skema .twscene + registry + façade client)   ← BARU
        │  load / step / inspect / state-diff / explain / checkpoint
        ▼
tw-sim ──► tw-core
```

Arah dependensi:

```text
tw-godot  →  tw-scene  →  tw-sim  →  tw-core
tw-cli    ──────────────►  tw-scene  →  tw-sim  →  tw-core
```

`tw-scene` adalah **API lapis** yang dipakai bersama oleh CLI dan Godot. Ini
membuat dua hal sekaligus:

- CLI tidak pernah menyentuh `Simulation` secara langsung → logika simulasi
  tetap di `tw-sim`, CLI tetap tipis (prinsip yang sudah dijaga di `README.md`).
- `tw-godot` tidak pernah menyentuh `Simulation` secara langsung → abstraksi
  tidak bocor, dan adapter Godot tetap tipis.

Alasan `tw-scene` menjadi crate terpisah (bukan module di `tw-sim`): ini adalah
"API" yang duduki sejajar dengan "CLI" pada diagram di `engine.md`
(`CLI | Library | API`). Kalau nanti `tw-scene` membengkak (banyak format,
banyak kind), memisahkannya sejak awal sudah benar. Kalau ternyata tetap kecil,
masih aman digabung — batas dependensi tetap terjaga.

---

## 5. Prasyarat: perubahan pada kode existing

Perubahan di bawah ini **tidak** menyentuh Godot sama sekali. Semua bisa
diverifikasi dengan `cargo test` dan CLI. Inilah yang membuat Fase A berdiri
sebagai lapisan netral, bukan sekadar " lessen Scout".

| # | File | Perubahan | Alasan |
| --- | --- | --- | --- |
| P1 | `tw-core/src/tracing/causal_graph.rs` | `TraceNode` menambah field `subject: Option<ComponentId>`; `CausalGraph` menambah indeks `ComponentId → Vec<TraceId>` dan method `traces_for()` / `last_trace_for()` | Godot harus bisa menjawab *"kenapa komponen ini berubah?"*. Sekarang `TraceNode` hanya tahu `event_id` + deskripsi, tidak tahu pemiliknya |
| P2 | `tw-core/src/state/mod.rs`, `event/mod.rs`, `time/`, `resource/`, `component/mod.rs` | Turunkan `derive(Serialize, Deserialize)` (tambah dependensi `serde`) | Checkpoint dan `.twscene` harus serializable. `serde` bersifat netral — tidak melanggar batas core |
| P3 | `tw-sim/src/simulation.rs` | Tambah `component_ids()`, `component_name(id)`, `component_ports(id)`, `resource_ids()`, `resource_snapshot(id)`; `step()` mengisi `subject` pada trace node | Façade butuh enumerasi, bukan cuma lookup per-ID. Godot butuh melihat port dan resource untuk menggambar inspector |
| P4 | `tw-sim/src/scenarios/*` | Pisahkan **"jenis komponen"** dari **"skenario utuh"**: `Counter::new`, `Gate::new`, `Worker`, `DiskJob`, `Probe` diekspos sebagai *kind* yang bisa di-instansiasi dengan parameter | `.twscene` butuh *factory per jenis*, bukan skenario yang sudah jadi. Skenario lama harus tetap jalan (regresi) |
| P5 | `tw-sim/src/scenarios/digital.rs` | Turunkan `PortId` dari `ComponentId`, bukan dari jenis komponen | **Rapuh, bukan bug hari ini.** `Gate::new()` memakai `kind.base()` (XOR→100, AND→110), sehingga dua instance jenis yang sama memakai `PortId` identik. Aman *hari ini* karena `Connection` memakai pasangan `(component_id, port_id)`, tetapi akan menyulitkan scene file yang menyimpan nama port per instance |
| P6 | `tw-cli` | Tambah `run --world <file>`, `inspect --world <file>`, `trace --world <file> --component <id>` | **Kunci Fase A**: seluruh kemampuan Godot bisa diuji tanpa Godot terpasang |

### 5.1 `EventPayload` tidak perlu dibuka

`EventPayload` yang tertutup **tidak** perlu diperluas untuk integrasi ini:

| Kebutuhan | Yang dipakai | Alasan |
| --- | --- | --- |
| Stimulus dari Godot ke port komponen | `EventPayload::PortSignal { port_id, value }` | Sudah ada |
| Nilai yang bisa dibuat/dibaca Godot | `StateValue` = `Bool / Int / Float / Text` | Memetakan 1:1 ke `Variant` Godot |
| Payload bebas | `EventPayload::Custom(String)` | Sudah ada |

Kalau nanti dibutuhkan payload biner (mis. data elektrik biner), baru ditambah
`EventPayload::Data(Vec<u8>)`. Itu keputusan fase lanjutan, bukan prasyarat.

---

## 6. Fase A — `tw-scene` (netral, tanpa Godot)

### 6.1 Isi crate

```text
crates/tw-scene/
├── Cargo.toml
├── src/
│   ├── lib.rs         # re-export
│   ├── world.rs       # skema .twscene (serde) + parsing + validasi
│   ├── registry.rs    # ComponentKind + factory + registry
│   ├── session.rs     # façade client
│   └── error.rs       # SceneError
└── tests/
    ├── world_parse.rs
    ├── registry.rs
    └── session.rs
```

### 6.2 Format dunia (`.twscene`)

Dokumen JSON. Skema:

```json
{
  "name": "traffic-demo",
  "ticks_per_second": 1000,
  "components": [
    {
      "id": 1,
      "kind": "Counter",
      "params": { "limit": 10, "interval": 2 }
    },
    {
      "id": 2,
      "kind": "Gate",
      "params": { "gate": "XOR" }
    }
  ],
  "connections": [
    {
      "from": { "component": 1, "port": "out" },
      "to":   { "component": 2, "port": "in_a" }
    }
  ],
  "resources": [
    { "id": 10, "name": "Disk", "capacity": 1, "service_time": 3 }
  ],
  "stimuli": [
    { "at": 0, "component": 1, "tick": true },
    { "at": 5, "component": 2, "port": "in_a", "value": 1 }
  ]
}
```

Penjelasan field:

| Field | Tipe | Arti |
| --- | --- | --- |
| `name` | string | Nama dunia (diagnostik) |
| `ticks_per_second` | u64 | Konversi tick ↔ detik dipakai adapter Godot |
| `components[].id` | u64 | `ComponentId` |
| `components[].kind` | string | Kunci di registry |
| `components[].params` | object | Parameter jenis, bebas bentuk; divalidasi oleh factory |
| `connections[].from/to` | `{component, port}` | **Nama** port, bukan `PortId` numerik |
| `resources[]` | — | Resource dengan kapasitas + latensi |
| `stimuli[]` | — | Event awal: `tick: true` (event tick generik) atau `port` + `value` (sinyal pada port) |

**Keputusan desain: port ditulis sebagai *nama*, bukan id numerik.**

Alasannya: `Component::ports()` mengembalikan `&[Port]` milik komponen itu
sendiri, jadi `Port` id adalah detail internal. Jika scene file menyimpan id
numerik, maka file itu terikat pada implementasi Rust dan rusak begitu port
di-refactor. Dengan nama, scene file stabil:

```text
scene file ──(nama)──► loader ──(resolusi via Component::ports())──► PortId
```

Konsekuensinya: `.twscene` **tidak bisa menciptakan port baru** pada komponen
generic. Port tetap milik "jenis" komponen. Ini memang desirable — `XOR` punya
`in_a/in_b/out_y`, `Counter` punya `tick/value` — dan menghindari perubahan
pada trait `Component`.

### 6.3 Registry jenis komponen

```rust
pub type ComponentFactory =
    fn(&ComponentSpec) -> Result<Box<dyn Component>, SceneError>;

pub struct Registry {
    kinds: HashMap<String, ComponentFactory>,
}

impl Registry {
    pub fn new() -> Self;
    pub fn builtin() -> Self;                       // jenis bawaan dari tw-sim
    pub fn register(&mut self, kind: &str, f: ComponentFactory);
}
```

Jenis bawaan (v1), semuanya sudah ada di `tw-sim/src/scenarios/`:

| Kind | Sumber | Param |
| --- | --- | --- |
| `Counter` | `scenarios::counter` | `limit`, `interval` |
| `Gate` | `scenarios::digital` | `gate` (`XOR` / `AND` / `OR`) |
| `Worker` | `scenarios::resource_contention` | `resource` |
| `DiskJob` | `scenarios::disk_service` | `resource` |
| `Probe` | `scenarios::digital` | — |

Jenis domain (CPU, RAM, transistor) menyusul setelah Phase 6+, dan akan
mendaftarkan diri ke registry yang sama — **tanpa mengubah** `tw-scene`,
`tw-sim`, atau `tw-godot`.

### 6.4 Façade client (`Session`)

```rust
pub struct Session {
    sim: Simulation,
    world: WorldMeta,
    prev_states: HashMap<ComponentId, State>,   // cache untuk deteksi perubahan
}

impl Session {
    // Pemuatan
    pub fn load(path: &Path) -> Result<Session, SceneError>;
    pub fn from_world(world: World, registry: Registry) -> Result<Session, SceneError>;

    // Waktu & eksekusi
    pub fn current_tick(&self) -> u64;
    pub fn advance_to(&mut self, tick: u64) -> FrameReport;
    pub fn step(&mut self, ticks: u64) -> FrameReport;

    // Inspeksi
    pub fn component_ids(&self) -> Vec<ComponentId>;
    pub fn component_name(&self, id: ComponentId) -> Option<&str>;
    pub fn component_state(&self, id: ComponentId) -> Option<&State>;
    pub fn component_ports(&self, id: ComponentId) -> Option<&[Port]>;
    pub fn resource_ids(&self) -> Vec<ResourceId>;
    pub fn resource_snapshot(&self, id: ResourceId) -> Option<&ResourceSnapshot>;

    // Perubahan state (untuk visual)
    pub fn take_state_changes(&mut self) -> Vec<StateChange>;

    // Kausalitas
    pub fn explain(&self, trace: TraceId) -> Vec<TraceNode>;
    pub fn traces_for(&self, component: ComponentId) -> Vec<TraceId>;
    pub fn last_trace_for(&self, component: ComponentId) -> Option<TraceId>;

    // Stimulus
    pub fn inject_port_signal(&mut self, id: ComponentId, port: &str, value: StateValue)
        -> Result<(), SceneError>;
    pub fn inject_tick(&mut self, id: ComponentId) -> Result<(), SceneError>;

    // Kontrol
    pub fn pause(&mut self);
    pub fn resume(&mut self);
    pub fn is_paused(&self) -> bool;
    pub fn set_tracing_mode(&mut self, mode: TracingMode);

    // Checkpoint
    pub fn checkpoint(&self) -> Checkpoint;
    pub fn restore(&mut self, checkpoint: &Checkpoint);
}

pub struct StateChange {
    pub component: ComponentId,
    pub key: String,
    pub old: Option<StateValue>,
    pub new: StateValue,
}

pub struct FrameReport {
    pub tick: u64,
    pub events_executed: u64,
    pub state_changes: Vec<StateChange>,
    pub new_traces: Vec<TraceId>,
}
```

**Deteksi perubahan state** dihitung di level `Session`: setelah `advance_to`,
`Session` membaca `component_state(id)` untuk tiap komponen dan membandingkannya
dengan cache `prev_states`. Konsekuensi:

- **Nol perubahan pada `tw-core`** untuk fitur ini.
- Biaya O(jumlah komponen) per frame — wajar untuk simulasi yang divisualisasi.
- Godot tidak perlu tahu *kapan* state berubah; cukup menarik diff per frame.

### 6.5 Verifikasi Fase A (tanpa Godot)

```bash
cargo test -p tw-scene
cargo run -p tw-cli -- run --world examples/traffic.twscene
cargo run -p tw-cli -- inspect --world examples/traffic.twscene
cargo run -p tw-cli -- trace --world examples/traffic.twscene --component 2
```

Kriteria selesai:

- [ ] `.twscene` parse + validasi (unknown kind, unknown port, duplicate id,
      unknown resource, param invalid → error yang jelas).
- [ ] Skenario lama (`run counter`, `run full-adder`, …) tetap hijau — regresi nol.
- [ ] `trace --world` mencetak rantai sebab-akibat dari `explain`.
- [ ] `take_state_changes()` menghasilkan diff yang benar pada simulasi
      yang mengubah state.
- [ ] Checkpoint → restore menghasilkan state identik.

---

## 7. Fase B — `tw-godot` (adapter GDExtension)

### 7.1 Bentuk crate

```text
crates/tw-godot/
├── Cargo.toml          # crate-type = ["cdylib"]
└── src/
    └── lib.rs          # class SimulationHost + entry point

godot/                            # project Godot untuk demo & smoke test
├── project.godot
├── addons/tw_godot/
│   ├── tw_godot.gdextension
│   └── bin/                    # hasil build .so
├── scenes/demo.tscn
└── tests/smoke.gd
```

`Cargo.toml`:

```toml
[package]
name = "tw-godot"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
tw-scene = { path = "../tw-scene" }
godot = { version = "0.5.5", features = ["api-4-7"] }
```

### 7.2 Kelas Godot

```rust
use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct SimulationHost {
    base: Base<Node>,
    session: Option<tw_scene::Session>,
    tick_accumulator: TickAccumulator,
}

#[godot_api]
impl SimulationHost {
    #[func]
    fn load_world(&mut self, path: GString) -> Error;

    #[func]
    fn start(&mut self);
    #[func]
    fn pause(&mut self);
    #[func]
    fn resume(&mut self);
    #[func]
    fn advance_ticks(&mut self, ticks: i64) -> i64;

    #[func]
    fn component_state(&self, id: i64) -> Dictionary;
    #[func]
    fn component_ports(&self, id: i64) -> Array;
    #[func]
    fn take_state_changes(&mut self) -> Array;

    #[func]
    fn explain(&self, trace_id: i64) -> Array;
    #[func]
    fn traces_for(&self, component_id: i64) -> Array;

    #[func]
    fn inject_signal(&mut self, component_id: i64, port: GString, value: Variant) -> Error;
    #[func]
    fn checkpoint(&self) -> Dictionary;

    #[func]
    fn set_tracing_mode(&mut self, mode: i64);
    #[func]
    fn set_ticks_per_second(&mut self, tps: i64);

    #[signal]
    fn state_changed(changes: Array);
    #[signal]
    fn event_executed(info: Dictionary);
    #[signal]
    fn trace_added(info: Dictionary);
}

struct TwGodot;

#[gdextension]
unsafe impl ExtensionLibrary for TwGodot {}
```

Berkas `.gdextension`:

```toml
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = "4.7"

[libraries]
linux.debug.x86_64   = "res://addons/tw_godot/bin/libtw_godot.linux.debug.x86_64.so"
linux.release.x86_64 = "res://addons/tw_godot/bin/libtw_godot.linux.release.x86_64.so"
macos.debug          = "res://addons/tw_godot/bin/libtw_godot.macos.debug.framework"
macos.release        = "res://addons/tw_godot/bin/libtw_godot.macos.release.framework"
windows.debug.x86_64 = "res://addons/tw_godot/bin/tw_godot.windows.debug.x86_64.dll"
windows.release.x86_64 = "res://addons/tw_godot/bin/tw_godot.windows.release.x86_64.dll"
```

### 7.3 Contoh pemakaian dari GDScript

```gdscript
var sim: SimulationHost

func _ready() -> void:
    sim = SimulationHost.new()
    add_child(sim)

    var err := sim.load_world("res://worlds/traffic.twscene")
    if err != OK:
        push_error("gagal memuat dunia")

    sim.ticks_per_second = 1000
    sim.tracing_mode = SimulationHost.TRACING_ABSTRACT
    sim.state_changed.connect(_on_state_changed)
    sim.start()

func _process(_delta: float) -> void:
    # State perubahan ditarik (pull), bukan didorong.
    for ch in sim.take_state_changes():
        _apply(ch)

func _apply(ch: Dictionary) -> void:
    var lamp: Node2D = get_node(ch["component"])
    lamp.modulate = ch["new"]
```

### 7.4 Permukaan API Godot

| Kategori | Item |
| --- | --- |
| Muatan | `load_world(path)`, `unload_world()` |
| Kontrol | `start()`, `pause()`, `resume()`, `stop()`, `advance_ticks(n)` |
| Waktu | `set_ticks_per_second(n)`, `current_tick()` |
| Inspeksi | `component_state(id) -> Dictionary`, `component_ports(id) -> Array`, `component_names() -> Array`, `resource_snapshots() -> Array` |
| Visual | `take_state_changes() -> Array` |
| Kausalitas | `explain(trace_id) -> Array`, `traces_for(component_id) -> Array` |
| Stimulus | `inject_signal(component_id, port, value)` |
| Checkpoint | `checkpoint() -> Dictionary` |
| Tracing | `set_tracing_mode(mode)` |
| Signal | `state_changed(changes)`, `event_executed(info)`, `trace_added(info)` |

### 7.5 Mapping tipe

| Engine | Godot | Catatan |
| --- | --- | --- |
| `StateValue::Bool` | `bool` | |
| `StateValue::Int` | `int` | `i64` |
| `StateValue::Float` | `float` | `f64` |
| `StateValue::Text` | `String` | |
| `State` | `Dictionary` | `String -> Variant` |
| `Vec<StateChange>` | `Array` of `Dictionary` | |
| `ComponentId` / `PortId` / `TraceId` / `ResourceId` | `int` | semuanya `u64` → `i64` |
| `Port` | `Dictionary` | `{id, name, direction}` |
| `TracingMode` | `int` konstanta | `TRACING_OFF/ABSTRACT/HIERARCHICAL` |

### 7.6 Mapping waktu

Godot memberi `delta` dalam **detik f64**. Engine memakai **tick u64**. Konversi
harus live di tepi Godot, bukan di dalam core (INVARIAN-4).

```text
TickAccumulator
  ─────────────────────────────────────────────
  tick_deadline_ns : u64   // tick berikutnya jatuh pada waktu ini
  tps              : u64   // ticks per second

  on_frame(delta_f64):
      now_ns += (delta_f64 * 1e9) as u64
      while now_ns >= tick_deadline_ns:
          session.advance_to(tick)
          tick += 1
          tick_deadline_ns = (tick * 1_000_000_000) / tps
```

Pendekatan "deadline per tick" dipilih, bukan akumulator pecahan, karena tidak
ada penumpukan galat pembulatan yang bertambah seiring waktu.

Konsekuensi yang perlu diterima:

- Simulasi **tetap deterministik**; yang non-deterministik hanya *kapan* frame
  Godot terjadi. Dua mesin dengan frame rate berbeda akan menjalankan jumlah
  tick yang berbeda pada detik yang sama, tetapi dengan urutan event identik.
- Untuk pengujian yang sepenuhnya deterministik, gunakan `advance_ticks(n)` —
  bukan `_process`.

### 7.7 Threading & reentrancy

Dua aturan keras, keduanya muncul dari sifat godot-rust:

**A. `Gd<T>` adalah `!Send` dan `!Sync`.** Objek Godot hanya boleh dipakai di
main thread. `Session` menyimpan `Simulation` yang `Box<dyn Component>`-nya
bukan `Send` — dan itu **cocok**, bukan masalah: engine memang dirancang
single-threaded. Konsekuensinya, simulasi tidak boleh dipindah ke worker thread
tanpa redesign (lihat §14).

**B. Jangan memanggil balik GDScript di tengah `#[func]` yang memegang borrow.**
godot-rust memakai *bind guard*; memanggil method pada objek Godot yang sedang
dipinjam akan **panic**. Aturan praktis:

```text
❌  #[func] fn advance(&mut self) {
        let changes = self.session.take_state_changes();   // borrow Session
        self.signals().state_changed().emit(changes);      // GDScript handler
    }                                                       // handler calling back
                                                        //   → panic double-borrow
```

Solusi yang dipakai: **model pull untuk state, model push hanya untuk kejadian
jarang**, dan pemanggilan ke GDScript selalu lewat `run_deferred` sehingga
berjalan setelah pemanggilan Rust selesai:

```rust
#[func]
fn advance_ticks(&mut self, ticks: i64) -> i64 {
    let report = self.session.step(ticks as u64);
    let count = report.events_executed as i64;
    // signal dikirim deferred → handler GDScript boleh memanggil balik
    self.base_mut().call_deferred("emit_state_changed", report.state_changes.to_godot());
    count
}
```

Untuk `state_changed`, model **pull** (`take_state_changes()`) sudah cukup dan
lebih aman — GDScript memanggil sendiri setiap frame. Signal dipakai untuk
`event_executed` dan `trace_added`, yang sifatnya jarang.

### 7.8 Verifikasi Fase B

```bash
cargo build -p tw-godot --release
cp target/release/libtw_godot.so \
   godot/addons/tw_godot/bin/libtw_godot.linux.release.x86_64.so

godot --headless --path godot --script tests/smoke.gd
```

Smoke test headless harus memaksa:

- [ ] `load_world` berhasil dan `component_names()` sesuai file.
- [ ] `advance_ticks(N)` menghasilkan `state_changes` non-kosong.
- [ ] `explain(trace_id)` mengembalikan rantai ≥ 1 node.
- [ ] `checkpoint()` → `restore()` → state identik.
- [ ] `inject_signal` mengubah state komponen tujuan pada tick berikutnya.

---

## 8. Fase C — editor (opsional)

Plugin GDScript `@tool` di `godot/addons/tw_godot/`:

| Fitur | Isi |
| --- | --- |
| Authoring | Inspector untuk menyusun `.twscene` (komponen, koneksi, resource) |
| Live preview | Scene demo yang merender state komponen secara langsung |
| Causality debug draw | Menggambar `CausalGraph` dari `explain()` sebagai node + edge |
| Checkpoint UI | Menyimpan/memuat snapshot dari editor |

Fase C **tidak mengubah** `tw-core`/`tw-sim`/`tw-scene` — seluruhnya di sisi
Godot.

---

## 9. Format checkpoint

`Checkpoint` sudah ada di `tw-sim` (`time`, `component_states`, `resources`) tetapi
hanya di memori. Dengan P2 (serde), ia bisa:

- diserialisasi ke JSON oleh `tw-scene` untuk disimpan dari editor Godot,
- dipulihkan ke `Session` kapan saja.

Godot juga punya serializer sendiri (`var_to_bytes` / `bytes_to_var`). Untuk v1,
disarankan **JSON via `tw-scene`** — satu format untuk CLI dan Godot, dan tetap
bisa dibaca manusia.

---

## 10. Rencana verifikasi (ringkas)

| Fase | Verifikasi | Cukup tanpa Godot? |
| --- | --- | --- |
| A | `cargo test -p tw-scene`, `cargo test --workspace`, CLI `--world` | Ya |
| B | `cargo build -p tw-godot`, `godot --headless --script tests/smoke.gd` | Tidak |
| C | Uji manual di editor | Tidak |

Aturan tambahan: setiap perubahan di `tw-core`/`tw-sim` harus tetap bisa
diverifikasi lewat CLI. Kalau sebuah fitur hanya bisa diuji lewat Godot, berarti
batas INVARIAN-5 mulai bocor.

---

## 11. Risiko

| Risiko | Dampak | Mitigasi |
| --- | --- | --- |
| MSRV: `godot` 0.5.5 butuh rustc ≥ 1.94, `stable` lokal 1.91.1 | Build `tw-godot` gagal | `rust-toolchain.toml` → `1.95.0` (sudah terpasang di mesin ini, tanpa unduhan). Alternatif: `godot` 0.4.5 (MSRV 1.87, API 4.2–4.5) |
| Godot belum terpasang di mesin | Smoke test tak bisa jalan | Unduh 1× (Godot 4.7.2 stable). **Build Rust tidak butuh binary Godot** — header API sudah dibundel di `godot-bindings` |
| Versi Godot pengguna ≠ target | GDExtension menolak dimuat | Pilih feature `api-4-2`…`api-4-7` sesuai versi target; `compatibility_minimum` di `.gdextension` harus sesuai |
| `TracingMode::Hierarchical` boros memori pada sesi panjang | OOM / FPS turun | Defaultkan `Abstract` di adapter; `Hierarchical` hanya on-demand saat menelusuri causality |
| `Box<dyn Component>` bukan `Send` | Tidak bisa dipindah ke worker thread | Diterima untuk v1 (cocok dengan desain engine). Jika butuh worker thread nanti: tambah bound `Send` di `tw-core`, refactor observer menjadi channel |
| Type Godot bocor ke core | Melanggar INVARIAN-1 | Guard di CI: `grep -rn "godot" crates/tw-core crates/tw-sim` harus kosong |
| Biaya FFI per event | FPS turun | Batch: satu panggilan `advance_ticks` per frame, bukan satu panggilan per event |

---

## 12. Fakta terverifikasi

Hasil pemeriksaan langsung terhadap repositori dan terhadap crate `godot`:

| Fakta | Status |
| --- | --- |
| `godot` 0.5.5 (terbaru, Agustus 2026) — MSRV **1.94**, mendukung `api-4-2`…`api-4-7` | terverifikasi |
| `godot` 0.4.5 — MSRV **1.87**, mendukung `api-4-2`…`api-4-5` | terverifikasi |
| `rustc` default di mesin ini = 1.91.1; toolchain **1.95.0 sudah terpasang** | terverifikasi |
| `Box<dyn Trait>` **tidak-`Send`** tetap bisa dipakai di dalam `#[derive(GodotClass)]` struct → **trait `Component` di `tw-core` tidak perlu diubah** | terverifikasi (kompilasi sukses) |
| Simbol entry point `gdext_rust_init` ter-export di `.so` | terverifikasi (`nm -D`) |
| Build crate `godot` **tidak** memerlukan binary Godot (header API dibundel) | terverifikasi (build berhasil tanpa Godot terpasang) |
| Godot stable saat ini = **4.7.2** (Agustus 2026); 4.8 masih beta | terverifikasi |
| `cargo test --workspace` baseline: 47 test hijau | terverifikasi |

---

## 13. Batasan yang diketahui

Fitur berikut **tidak** ada di engine saat ini, sehingga **tidak** boleh
diekspos ke Godot sampai dibangun:

- **Multi-fidelity / refinement** (`refine()`, `select_fidelity()`). Belum ada
  di engine — lihat `implementation.md` §"Batas & yang belum dikerjakan".
- **Fondasi numerik** (`tw-num`): matriks, solver linear/nonlinear, DAE. Level
  fidelity electrically belum bisa dibandingkan.
- **Model domain**: `models/` masih placeholder; belum ada CPU/RAM/transistor.

Kalau UI Godot sudah siap, operasi ini sebaiknya **tidak** ditampilkan dulu,
atau ditampilkan dalam keadaan nonaktif yang jujur — bukan tombol yang diam-diam
tidak melakukan apa pun.

---

## 14. Arah lanjut yang belum diputuskan

| Topik | Opsi | Rekomendasi |
| --- | --- | --- |
| Simulasi di thread terpisah | (a) main thread saja; (b) worker thread + channel | (a) untuk v1. Butuh bound `Send` di `tw-core` bila (b) dipilih |
| Ukuran data per frame | (a) kirim seluruh state tiap frame; (b) kirim diff | (b) — sudah dirancang di `Session::take_state_changes()` |
| Who/how authoring `.twscene` | (a) teks biasa; (b) plugin editor | (a) dulu, (b) Fase C |
| Format checkpoint | (a) JSON; (b) biner Godot | (a) — satu format untuk CLI + Godot |

---

## 15. Keputusan terbuka

Dua hal harus diputuskan sebelum Fase B dimulai:

1. **Target versi Godot.** 4.7.2 (stable terbaru, `api-4-7`, perlu unduh
   binary) atau 4.4 (`api-4-4`, lebih luas dipakai)?
2. **Toolchain.** Naikkan workspace ke `1.95.0` (memakai `godot` 0.5.5, API
   terbaru) atau pertahankan `stable` 1.91.1 + `godot` 0.4.5 (API 4.2–4.5)?

Rekomendasi: **Godot 4.7.2 + toolchain 1.95.0 + `godot` 0.5.5** — semua sudah
tersedia di mesin ini, dan tidak membatasi versi Godot ke belakang.

---

## 16. Estimasi & urutan

| Fase | Cakupan | Estimasi |
| --- | --- | --- |
| **A** | P1–P6 + crate `tw-scene` + CLI `--world` | 2–3 hari |
| **B** | crate `tw-godot` + `.gdextension` + project Godot + smoke test headless | 1–2 hari (+ unduh Godot) |
| **C** | plugin editor, live preview, debug-draw causality | 2–4 hari |

Urutannya tegas: **A selesai dulu**, karena `.twscene` + `Session` adalah kontrak
yang dipakai bersama oleh CLI dan Godot. Kalau B dimulai lebih dulu, kontrak itu
belum ada dan akan lahir dua kali.

---

## Referensi

- `engine.md` — roadmap alternatif; **Phase 12 — External Interface / Godot**.
- `engine-v1.md` — roadmap bertahap; **Phase 16 — External API / Godot**;
  definisi operasi client Godot.
- `plan.md` — desain arsitektur & sistem; model komposisi dan refinement.
- `implementation.md` — apa yang benar-benar terimplementasi, dan batasnya.
- `time-model.md` — model waktu (tick fixed-point), batas, opsi sinkronisasi.
- `project-structure.md` — struktur direktori yang direncanakan.
- `numerical/overview.md` — peta fondasi numerik (Phase 6).
- `godot-rust` book — <https://godot-rust.github.io/book/>
- crate `godot` — <https://crates.io/crates/godot>
