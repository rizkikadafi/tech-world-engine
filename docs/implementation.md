# Implementasi Fondasi Engine (P0 → P2)

Dokumen ini menjelaskan **apa yang benar-benar diimplementasikan** pada tiga commit
terakhir repositori, bukan sekadar rencana. Fokusnya: keputusan desain, bentuk API,
dan cara memverifikasinya.

Rencana lengkap (visi) ada di `engine.md`, `engine-v1.md`, dan `plan.md`; dokumen
ini adalah catatan **hasil** yang memetakan implementasi ke visi tersebut.

## Tiga commit terakhir

| Commit | Judul | Cakupan | Test hijau |
| --- | --- | --- | ---: |
| `692a686` | Foundation: complete Phase 0 scaffolding (P0 batch) | P0.1–P0.4, P0.5 (bonus), P2.1 | 24 |
| `09dd5d6` | Design debt: complete P1 batch | P1.1–P1.5 | 43 |
| `60a82a3` | Hygiene: complete P2 batch | P2.2–P2.4 | 47 |

Urutan eksekusinya sengaja: perbaikan berisiko rendah dulu, lalu test, lalu
pembersihan desain, terakhir higiene/benchmark.

## Peta ke roadmap

```text
engine-v1.md                        commit
─────────────────────────────────────────────────────────────
Phase 0  Project Foundation   ──►  692a686  (workspace, CLI, test, scenario)
Phase 1  Simulation Kernel    ──►  sudah ada → dirapikan di 692a686/09dd5d6
Phase 2  Component/Composition──►  09dd5d6  (validate + lifecycle)
Phase 3  Event & Time         ──►  692a686  (EventId otoritatif)
Phase 4  Observation/Tracing  ──►  692a686  (TracingMode::Off) + 09dd5d6 (explain)
Phase 5  Generic Resource     ──►  09dd5d6  (Amount, policy, service_time)
Phase 6  Numerical Foundation ──►  belum (langkah berikutnya)
```

## Invarian yang dijaga

Semua commit mematuhi aturan yang menjadi inti proyek:

1. **`tw-core` domain-agnostic.** Tidak ada CPU/RAM/NAND/Transistor di core.
   Konsep domain hanya boleh hidup di `models/` (lihat `models/README.md`).
2. **Arah dependency satu arah:** `tw-cli → tw-sim → tw-core`.
3. **Satu sumber wiring.** CLI, test, dan example memakai `tw_sim::scenarios`
   yang sama; CLI tidak memuat logika simulasi.
4. **Engine otoritatif atas `EventId`.** Komponen tidak boleh mengarang id.
5. **Determinisme.** Urutan event stabil dan dapat direproduksi.
6. **State bertipe.** Tidak ada lagi `parse()` di example.

---

# Commit `692a686` — Fondasi (Phase 0)

## 1. `tw-sim::scenarios` — satu sumber wiring

Masalahnya: sebelumnya setiap example dan test menyusun ulang komponen, koneksi,
dan event sendiri, sehingga mudah menyimpang. CLI pun ikut memuat logika simulasi.

Solusinya modul `tw_sim::scenarios` yang **side-effect free** (tidak mencetak
apa pun) dan mengekspor fungsi `build()`/runner yang dipakai bersama:

```text
tw-sim/src/scenarios/
├── mod.rs                 # dokumentasi & re-export
├── counter.rs             # komponen generik Counter
├── digital.rs             # gate/probe + Full Adder (composite)
├── resource_contention.rs # 3 worker berbagi resource capacity 2
└── disk_service.rs        # (ditambahkan di commit berikutnya)
```

Contoh API:

```rust
// counter.rs
pub const COUNTER_ID: ComponentId = ComponentId(1);
pub fn build(limit: u64, interval: u64) -> Simulation;

// digital.rs
pub fn create_full_adder() -> CompositeComponent;
pub fn run_full_adder(a: u8, b: u8, cin: u8, mode: TracingMode) -> SimulationEngine;
pub fn expected_full_adder(a: u8, b: u8, cin: u8) -> (u8, u8);

// resource_contention.rs
pub const RESOURCE_ID: ResourceId = ResourceId(100);
pub fn build() -> Simulation;
```

Konsekuensinya, example di `crates/tw-sim/examples/` kini tipis: cukup memanggil
scenario lalu mencetak hasil. Test integrasi (`crates/tw-sim/tests/scenarios.rs`)
menguji wiring yang sama.

## 2. `EventId` otoritatif

Sebelumnya komponen bisa membuat id sendiri, misalnya
`EventId(event.id.0 + 10)`. Id semacam itu bisa **bertabrakan** dengan id engine,
dan karena antrian event diurutkan berdasarkan `(time, EventId)`, tabrakan
merusak determinisme pada tick yang sama.

Sekarang engine yang menerbitkan id:

```rust
impl Simulation {
    fn next_id(&mut self) -> EventId {           // counter internal
        self.next_event_id += 1;
        EventId(self.next_event_id)
    }

    pub fn schedule_event(&mut self, mut event: Event) {
        event.id = self.next_id();               // id komponen ditimpa
        self.event_queue.push(ScheduledEvent(event));
    }
}
```

Komponen memakai konstruktor placeholder:

```rust
impl Event {
    /// id = EventId(0); engine mengisi id asli saat penjadwalan.
    pub fn at(time, source, target, payload) -> Self;
}
```

Event internal engine (`ResourceGranted`, forwarding port, dsb.) dibuat lewat
`push_internal`, sehingga semuanya punya id unik. Antrian `BinaryHeap` mengurutkan
`time` lalu `id`, jadi dua event pada tick yang sama tetap diproses dengan urutan
yang sama di setiap run.

## 3. `TracingMode::Off`

Tracing kini punya tiga tingkat:

```rust
pub enum TracingMode {
    Off,          // tidak ada node; cocok untuk simulasi besar
    #[default]
    Abstract,     // satu node per event yang dieksekusi
    Hierarchical, // + node untuk routing internal composite
}
```

`step()` hanya mencatat node jika `is_enabled()`, dan composite hanya mencatat
routing internal jika `is_hierarchical()`. Bukti pada Full Adder:

```text
TracingMode::Off          →  0 node
TracingMode::Abstract     → 11 node
TracingMode::Hierarchical → 74 node
```

Dengan `Off`, memori tidak tumbuh mengikuti jumlah event — prasyarat Phase 11
(scaling) tanpa membuang observasi/event log.

## 4. CLI

`tw-cli` hanya antarmuka:

```bash
tw-cli run <scenario>      # jalankan & cetak hasil
tw-cli inspect <scenario>  # jalankan & cetak state komponen
tw-cli list                # daftar skenario
```

Skenario awal: `counter`, `resource-contention`, `full-adder`.

## 5. Pembersihan dead code (P2.1)

`examples/counter.rs` di root memakai signature trait lama (`handle_event(&Event)`
tanpa `ctx`, tanpa `ports()`) dan berada di luar workspace sehingga tidak pernah
dikompilasi. File dihapus; satu-satunya sumber contoh adalah
`crates/tw-sim/examples/`.

## Verifikasi commit ini

- `cargo test` → 24 hijau (tw-core: `time_state`, `resource`, `causal_graph`;
  tw-sim: `scenarios`).
- Test kunci: `schedule_event_assigns_unique_engine_ids`,
  `events_are_processed_in_time_order`, `simulation_is_deterministic_across_runs`,
  `tracing_off_records_no_nodes`, `hierarchical_mode_records_internal_routing`,
  `full_adder_truth_table_is_correct`.

---

# Commit `09dd5d6` — Design Debt (Phase 1)

## P1.1 — State bertipe

`State` dulu hanya wadah nilai tanpa tipe yang kuat. Kini ada enum nilai:

```rust
pub enum StateValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}
```

`State` menyediakan aksesor bertipe (`get_bool`, `get_int`, `get_float`, `get_str`,
`get`) plus `From<..>` untuk tipe umum. `EventPayload::PortSignal` dan
`SignalChanged` kini membawa `StateValue`, bukan string:

```rust
PortSignal { port_id: PortId, value: StateValue }
```

**Kenapa enum, bukan `Box<dyn Any>`?** `Box<dyn Any>` akan menghilangkan `Debug`
dan `Clone` pada `Event`/`State` yang dibutuhkan observer, checkpoint, dan pesan
error. Enum kecil sudah cukup untuk kebutuhan saat ini dan tetap bisa diperluas.

Karena nilai sudah bertipe, example tidak lagi memakai `parse()` untuk membaca
hasil; cukup `state.get_int("sum")`.

## P1.2 — Snapshot & checkpoint

`Observer` punya dua hook:

```rust
pub trait Observer: Debug {
    fn on_event(&mut self, event: &Event);
    fn on_snapshot(&mut self, snapshot: &Snapshot);   // Snapshot { time, state }
}
```

`Simulation::checkpoint()` memanggil `on_snapshot` untuk setiap komponen dan
mengembalikan `Checkpoint`:

```rust
pub struct Checkpoint {
    pub time: SimulationTime,
    pub component_states: Vec<(ComponentId, State)>,
    pub resources: Vec<ResourceSnapshot>,
}

impl Simulation {
    pub fn checkpoint(&mut self) -> Checkpoint;
    pub fn restore_resources(&mut self, checkpoint: &Checkpoint);
}
```

Catatan desain: clock **tidak** dipulihkan oleh restore — waktu hanya dimajukan
oleh event loop, bukan oleh restore. Komponen yang ingin bisa di-restore penuh
harus menyimpan state bermaknanya di `State`.

## P1.3 — Validasi composite + lifecycle

`CompositeComponent` kini punya `validate()` yang mengumpulkan **semua** masalah
wiring sekaligus:

```rust
pub enum CompositeError {
    UnknownExternalPort { external_port: PortId },
    UnknownSubcomponent { component: ComponentId },
    UnknownSubcomponentPort { component: ComponentId, port: PortId },
    DirectionMismatch { external_port, internal_component, internal_port },
    DuplicateBinding { external_port, internal_component, internal_port },
    UnknownConnectionEndpoint { component: ComponentId, port: PortId },
}

impl CompositeComponent {
    pub fn validate(&self) -> Result<(), Vec<CompositeError>>;
}
```

Di samping itu ada lifecycle hook default pada trait `Component`:

```rust
fn on_mount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> { Vec::new() }
fn on_unmount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> { Vec::new() }
```

`on_mount` dipanggil dari `register_component`, `on_unmount` dari `shutdown`;
event yang dikembalikan otomatis dijadwalkan. Ini memberi komponen tempat
inisialisasi/penutupan tanpa "aturan khusus" di engine.

Penting: **hierarki hidup di dalam composite.** Engine hanya melihat komponen
top-level. Composite-lah yang merutekan event ke subkomponen lewat
`Connection` + `PortBinding`, lalu mengangkat output-nya kembali keluar.

## P1.4 — `CausalGraph::explain()`

`explain(target)` mengembalikan seluruh leluhur kausal (termasuk `target`) dengan
dua jaminan:

1. **Dedup** — setiap `TraceNode` muncul tepat sekali (dulu bisa berulang pada
   graf berbentuk diamond).
2. **Urut topologis** — diurutkan berdasarkan `TraceId`, yaitu urutan pembuatan;
   karena sebuah cause selalu dicatat sebelum akibatnya, urutan id selalu
   topologis (cause sebelum effect).

```rust
let chain = engine.simulation().explain(trace_id);
```

Test `causal_graph.rs` memakai graf diamond untuk membuktikan keduanya.

## P1.5 — Resource: `Amount`, policy, dan latency

Resource tidak lagi terpaku pada `u64`:

```rust
pub enum Amount {
    Discrete(u64),    // mesin, koneksi, slot
    Continuous(f64),  // bandwidth, daya, memori
}
```

`Amount` punya operasi aman (`checked_sub`, `checked_add`, `min_with`) dan
menolak pencampuran jenis. Request yang jenisnya tidak cocok ditolak.

Keputusan siapa yang dilayani **dikeluarkan dari engine** lewat trait pluggable:

```rust
pub trait AllocationPolicy: Debug {
    fn select(&self, available: &Amount, queue: &VecDeque<PendingRequest>) -> Vec<usize>;
}

pub struct FifoPolicy;      // tidak ada yang disalip
pub struct PriorityPolicy;  // prioritas menurun, tidak saling memblokir
```

`Resource::with_policy(...)` memasang policy, `Resource::with_service_time(n)`
memberi latency. Yang terakhir inilah kuncinya: ketika sebuah alokasi diberikan,
engine otomatis menjadwalkan pelepasan kapasitas `n` tick kemudian
(`grant_and_schedule`). Tidak ada satu pun aturan domain seperti
`if disk_busy: slow_down()`.

Hasilnya latency **emergent** dari posisi antrean. Scenario `disk-service`
(capacity 1, `SERVICE_TIME = 3`, tiga job tiba pada tick 0/1/2):

| Job | arrived | granted | released | latency |
| --- | ---: | ---: | ---: | ---: |
| Job1 | 0 | 0 | 3 | 0 |
| Job2 | 1 | 3 | 6 | 2 |
| Job3 | 2 | 6 | 9 | 4 |

Test terkait:

- `crates/tw-core/tests/resource.rs` — FIFO, priority, Amount, invalid request.
- `crates/tw-core/tests/composite_validation.rs` — 7 kasus error wiring.
- `crates/tw-sim/tests/disk_service.rs` — latency & pelepasan otomatis.
- `crates/tw-sim/tests/lifecycle.rs` — `on_mount`/`on_unmount`.
- `crates/tw-sim/tests/checkpoint.rs` — snapshot + restore resource.

> Catatan: `disk_service.rs` sendiri baru dibuat di commit `09dd5d6`, sehingga
> skenario ini menjadi bukti utama P1.5.

---

# Commit `60a82a3` — Higiene (Phase 0 tooling)

## P2.2 — Konfigurasi, logging, error type

### Configuration

```rust
pub struct SimulationConfig {
    pub tracing_mode: TracingMode,
    pub max_events: Option<u64>,  // batas aman jumlah event dieksekusi
    pub seed: u64,                // reserved untuk model stokastik
}

impl Simulation {
    pub fn with_config(config: SimulationConfig) -> Self;
    pub fn configure(&mut self, config: SimulationConfig);
    pub fn config(&self) -> SimulationConfig;
    pub fn events_processed(&self) -> u64;
}
```

`max_events` adalah pengaman anti-loop-tak-berujung: bila batas tercapai,
`step()` mengembalikan `None` dan **semua** run-loop berhenti
(`run_until` memeriksa hasil `step()`).

### Logging

Engine memakai facade `log` (bukan `println!`): `trace!` untuk tiap event yang
dieksekusi, `warn!` saat batas `max_events` tercapai. CLI menyalakan `env_logger`,
jadi level diatur lewat `RUST_LOG`:

```bash
RUST_LOG=tw_sim=trace cargo run -p tw-cli -- run counter
```

### Error type

`String` alasan diganti tipe error nyata (`thiserror`):

```rust
#[derive(thiserror::Error)]
pub enum ResourceError {
    #[error("requested amount is {actual} but resource capacity is {expected}")]
    KindMismatch { actual: &'static str, expected: &'static str },
    #[error("requested amount {requested} exceeds total resource capacity {capacity}")]
    ExceedsCapacity { requested: String, capacity: String },
}

pub enum ResourceRequestResult {
    Granted,
    Queued { queue_position: usize },
    Invalid { reason: ResourceError },
}
```

### Konfigurasi CLI

CLI membaca environment tanpa memuat logika simulasi:

| Variabel | Arti | Default |
| --- | --- | --- |
| `RUST_LOG` | Level log engine | off |
| `TW_MAX_EVENTS` | Batas jumlah event dieksekusi | tak terbatas |
| `TW_SEED` | Seed (reserved) | `0` |

```bash
TW_MAX_EVENTS=5 cargo run -p tw-cli -- run counter   # berhenti di 5 event
```

## P2.3 — Benchmark (criterion)

`crates/tw-sim/benches/scaling.rs` mengukur throughput event dengan komponen
10 / 100 / 1K / 10K (tiap komponen mengeksekusi satu `Tick`):

```bash
cargo bench -p tw-sim --bench scaling
```

Baseline (sample-size 10, mesin developer):

| Komponen | Waktu | Throughput |
| ---: | ---: | ---: |
| 10 | ~14 µs | ~720 Kelem/s |
| 100 | ~102 µs | ~0.98 Melem/s |
| 1K | ~1.8 ms | ~0.55 Melem/s |
| 10K | ~18 ms | ~0.54 Melem/s |

Angka ini menjadi titik awal trade-off fidelity vs performa (Phase 11).

## P2.4 — Struktur folder & README

- `README.md` root: peta crate, arah dependency, quick start, konfigurasi, benchmark.
- `models/README.md`: placeholder domain model + aturan bahwa engine harus tetap
  kompilasi/teruji walau folder ini kosong.
- `crates/tw-sim/src/config.rs`: modul config tersendiri (bukan menumpuk di
  `simulation.rs`).

---

# Cara memverifikasi seluruh fondasi

```bash
cargo build --all-targets
cargo test                       # 47 test
cargo clippy --all-targets       # bersih
cargo bench -p tw-sim --bench scaling

cargo run -p tw-cli -- list
cargo run -p tw-cli -- run counter
cargo run -p tw-cli -- inspect full-adder
```

Sebaran test saat ini:

```text
tw-core  29  (causal_graph 5, composite_validation 7, resource 11, time_state 6)
tw-sim   18  (scenarios 8, checkpoint 2, config 4, lifecycle 2, disk_service 2)
total    47
```

---

# Batas & yang belum dikerjakan

Ini penting agar ekspektasi selaras dengan roadmap:

- **Phase 6 — Numerical Foundation belum ada.** Belum ada matriks, representasi
  sparse, maupun solver linear/nonlinear. `StateValue::Float` sudah tersedia
  sebagai fondasi, tetapi engine belum melakukan komputasi numerik.
- **Belum ada model domain.** `models/` masih placeholder; belum ada CPU/RAM/
  transistor. Itu memang di luar core hingga Phase 6+.
- **Belum ada multi-fidelity/refinement** (Phase 12–13).
- **`seed` belum dipakai.** Disimpan untuk model stokastik di masa depan.
- **`Amount::Continuous`** sudah didukung tipe, tetapi skenario yang ada masih
  memakai `Discrete`.
- **`max_events` bersifat kasar** (per jumlah event, bukan per waktu) — cukup
  sebagai pengaman, bukan penjadwal.

# Referensi

- `engine-v1.md` — roadmap implementasi bertahap (Phase 0–16).
- `engine.md` — roadmap alternatif + milestone.
- `plan.md` — desain arsitektur & sistem.
- `general-simulation-core.md` — dasar teoritis (state transition, DES, tracing).
- `resource.md` — filosofi sistem resource.
- `project-structure.md` — struktur crate & dependency.
- `models/README.md` — aturan model domain.
