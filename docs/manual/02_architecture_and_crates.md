# Bab 02: Arsitektur Sistem & Crate Reference

Tech World Engine disusun sebagai **Cargo Workspace modular** di mana setiap crate memiliki tanggung jawab tunggal (*single responsibility*) dengan batasan dependensi yang sangat ketat.

---

## 1. Peta Dependensi Crate (*Dependency Graph*)

Arah ketergantungan antar-crate bersifat **satu arah ke bawah**:

```text
       ┌───────────┐         ┌────────────────┐
       │  tw-cli   │         │ Godot 4 Client │
       └─────┬─────┘         └───────┬────────┘
             │                       │ (via GDExtension)
             ▼                       ▼
       ┌──────────────────────────────────────┐
       │               tw-scene               │ (World .twscene, Registry, Session)
       └──────────────────┬───────────────────┘
                          │
                          ▼
       ┌──────────────────────────────────────┐
       │                tw-sim                │ (Simulation Loop, Wiring, Checkpoint)
       └──────────────────┬───────────────────┘
                          │
                          ▼
       ┌──────────────────────────────────────┐
       │               tw-core                │ (Domain-Agnostic Kernel Primitives)
       └──────────────────▲───────────────────┘
                          │
       ┌──────────────────┴───────────────────┐
       │          models/electrical           │ (Circuit, MNA, CMOS, Multi-Fidelity)
       └──────────────────┬───────────────────┘
                          │
                          ▼
       ┌──────────────────────────────────────┐
       │                tw-num                │ (Matrix, Sparse LU, Newton-Raphson)
       └──────────────────────────────────────┘
```

Lapisan authoring rangkaian berada di atas domain fisika:

```text
tw-schematic ──► models/electrical (tw-electrical) ──► tw-num / tw-core
```

`tw-schematic` menyimpan dokumen yang bisa diedit (part, pin, wire, posisi,
command/undo); `tw-electrical` tetap menjadi satu-satunya pemilik solver dan
netlist hasil compile.

> [!IMPORTANT]
> **Aturan Dependensi:**
> 1. `tw-core` tidak pernah mengimpor crate mana pun di workspace ini.
> 2. `tw-sim` hanya bergantung pada `tw-core`.
> 3. `tw-scene` menjembatani `tw-sim` dan `tw-core` ke dunia luar; tidak pernah mengimpor `tw-cli` atau `godot`.
> 4. `models/electrical` bergantung pada `tw-core` dan `tw-num`, namun `tw-core` tidak tahu tentang keberadaan sirkuit atau transistor.

---

## 2. Rincian Peran Masing-Masing Crate

### A. `crates/tw-core` — Kernel Primitif Generik
Fondasi murni simulasi diskrit. Tidak memiliki loop simulasi dan tidak memajukan waktu sendiri.

* **Modul Waktu ([`time`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/time/simulation_time.rs))**:
  - `SimulationTime`: Wrapper integer `u64 ticks`. Deterministik dan bebas rounding error.
* **Modul State ([`state`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/state/mod.rs))**:
  - `StateValue`: Nilai skalar bertipe (`Bool`, `Int`, `Float`, `Text`). Menggunakan `#[serde(untagged)]` sehingga natural di format JSON.
  - `State`: Wadah hashmap variabel terdaftar yang dimiliki oleh komponen.
* **Modul Event ([`event`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/event/mod.rs))**:
  - `EventId`: Pengenal unik otoritatif yang diterbitkan oleh engine.
  - `EventPayload`: Sinyal port (`PortSignal`), pemicu periodik (`Tick`), interaksi resource (`ResourceRequested`, dll), atau string bebas (`Custom`).
  - `Event`: Membawa metadata `id`, `time`, `source`, `target`, `payload`, dan `cause` (TraceId).
* **Modul Komponen ([`component`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/component/mod.rs))**:
  - `trait Component`: Kontrak utama yang diimplementasikan oleh setiap objek di engine.
  - `Port`, `PortId`, `PortDirection` (`Input`, `Output`, `Bidirectional`).
  - `CompositeComponent`: Komposisi yang menampung subkomponen dan merutekan koneksi secara internal.
* **Modul Resource ([`resource`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/resource/mod.rs))**:
  - `Resource`: Pengatur kapasitas fisik bersama dengan antrean alokasi (`FifoPolicy`, `PriorityPolicy`).
  - `Amount`: Kapasitas diskrit (`Discrete(u64)`) atau kontinu (`Continuous(f64)`).
* **Modul Kausalitas ([`tracing`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/tracing/causal_graph.rs))**:
  - `CausalGraph`: Graf berarah tanpa siklus (DAG) yang merekam penyebab setiap event yang dieksekusi.
  - Menyediakan indeks `traces_for(ComponentId)` dan algoritma `explain(TraceId)` yang terurut topologis tanpa duplikasi node.

---

### B. `crates/tw-sim` — Simulation Runner & Lifecycle
Engine yang memegang koleksi komponen dan memutar antrean prioritas waktu (*event loop*).

* **`Simulation`**:
  - Memegang antrean biner prioritas (`BinaryHeap<ScheduledEvent>`).
  - Menerbitkan `EventId` baru yang selalu increment dan stabil.
  - Mengelola lifecycle: memanggil `on_mount` saat registrasi komponen, dan `on_unmount` saat shutdown.
  - Menyediakan validasi koneksi (`validate_wiring`) untuk mendeteksi port hilang, arah salah, atau ID tidak dikenal.
  - Menyediakan API refleksi: `component_ids()`, `component_name()`, `component_ports()`, `port_by_name()`.
  - Checkpoint & restore engine: snapshot state semua komponen dan antrean resource via `checkpoint()`, serta restorasi via `restore()`.
* **`SimulationEngine`**:
  - Wrapper tipis untuk menjalankan simulasi selama $N$ ticks (`run_ticks(N)`).
* **`scenarios`**:
  - Koleksi implementasi komponen dasar yang dapat digunakan kembali: `Counter`, `Gate` (digital logic), `Probe`, `WorkerProcess`, dan `DiskJob`.

---

### C. `crates/tw-scene` — Lapisan Deklaratif & Client Façade
Lapisan abstraksi tingkat tinggi yang membuat engine dapat dikonsumsi oleh konfigurasi teks dan klien antarmuka tanpa menyentuh internal engine.

* **Format Dunia ([`world.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-scene/src/world.rs))**:
  - Struktur data serialisasi JSON `.twscene`: mendefinisikan komponen, parameter, koneksi port bernama, resource, dan stimulus awal.
* **Component Registry ([`registry.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-scene/src/registry.rs))**:
  - Tempat mendaftarkan pembuat (*factory*) komponen berdasarkan string nama `kind`.
  - Menyediakan `Registry::builtin()` untuk komponen standar.
* **Client Façade `Session` ([`session.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-scene/src/session.rs))**:
  - Antarmuka utama yang digunakan oleh CLI dan Godot:
    - `step(ticks) -> FrameReport`: Memajukan simulasi sejumlah tick.
    - `take_state_changes() -> Vec<StateChange>`: Menghasilkan diff perubahan variabel sejak frame sebelumnya (kunci untuk visualisasi rendering 60 FPS).
    - `inject_port_signal()`, `inject_tick()`: Memberikan stimulus interaktif saat simulasi berjalan.
    - `explain()`, `traces_for()`: Mengakses grafik kausalitas.

---

### D. `crates/tw-cli` — Command Line Interface
Klien terminal mandiri. Tidak memiliki logika simulasi internal; semua pemuatan dan eksekusi diserahkan ke `tw-scene` dan `tw-sim`.

* `tw-cli run <file.twscene>`: Memuat scene, mengeksekusi $N$ tick, dan mencetak state akhir.
* `tw-cli inspect <file.twscene> [--component ID]`: Memeriksa detail port dan variabel seluruh komponen.
* `tw-cli trace <file.twscene> --component ID`: Menjelaskan rantai kausalitas penyebab perubahan komponen.
* `tw-cli list-kinds`: Menampilkan daftar jenis komponen yang terdaftar di registry.

---

### D2. `crates/tw-godot` — Adapter GDExtension
`cdylib` tipis yang mengekspos `tw-scene::Session` sebagai node Godot 4 bernama `SimulationHost`.

* **Tanpa logika simulasi**: hanya memuat world, mengubah `delta` detik menjadi tick integer, memetakan `StateValue`/`Port`/`TraceNode` ke `Variant`, dan meneruskan stimulus.
* **Model pull**: GDScript memanggil `take_state_changes()` setiap frame; tidak ada pemanggilan balik ke GDScript dari dalam `#[func]` yang memegang borrow.
* **Checkpoint JSON**: `checkpoint_json()` / `restore_json()` berbagi format dengan CLI.
* **Project demo**: `godot/` berisi `project.godot`, skena demo, dan smoke test headless.

---

### D3. `crates/tw-schematic` — Lapisan Authoring Rangkaian
Dokumen rangkaian yang bisa diedit pemain. Lapisan ini **tidak memiliki fisika**: ia hanya menyimpan part, pin, wire, posisi 2D, dan riwayat edit, lalu mengompilasinya menjadi `tw-electrical::Circuit` untuk disolver.

* **Model dokumen ([`document.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-schematic/src/document.rs))**:
  - `CircuitDocument` menyimpan `parts`, `wires`, nama, dan `version: 1` (JSON stabil untuk save/load).
  - Part bawaan: resistor, kapasitor, sumber tegangan/arus, ground, dioda, NMOS/PMOS, lampu, saklar, dan probe.
  - `Wire` menghubungkan dua `PinRef`; titik routing hanya data presentasi dan tidak memengaruhi konektivitas.
* **Resolusi net ([`net.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-schematic/src/net.rs))**:
  - Union-find atas seluruh pin menghasilkan `NetId` deterministik; semua pin ground dilebur ke `NetId::GROUND`.
* **Validasi ([`validate.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-schematic/src/validate.rs))**:
  - Error: ID duplikat, pin/part tak dikenal, self-wire, parameter tidak valid, ground hilang/tak terhubung, sumber ter-short.
  - Warning: pin menggantung dan koneksi duplikat (lazim saat pengguna masih menggambar).
* **Kompilasi ([`compile.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-schematic/src/compile.rs))**:
  - Memetakan net ke `NodeId`, memanggil API netlist `tw-electrical`, dan menyimpan `CompiledCircuit` berisi pemetaan pin→node serta probe untuk query tegangan.
  - Lampu dan saklar dikompilasi sebagai resistor; probe hanya merekam node.
* **Command & sesi ([`command.rs`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-schematic/src/command.rs))**:
  - `Command` (`AddPart`, `RemovePart`, `MovePart`, `SetParam`, `RenamePart`, `Connect`, `Disconnect`) dengan undo/redo.
  - `SchematicSession` menyimpan cache compile dan state DC; setiap edit menaikkan `revision` dan membatalkan cache.

---

### E. `crates/tw-num` — Fondasi Matematika Numerik
Domain-agnostic numerical mathematics. Tidak memiliki dependensi ke konsep kelistrikan.

* Matriks dense dan vektor.
* Faktorisasi LU dense dengan partial pivoting.
* Matriks sparse (Compressed Sparse Column / triplet format) dan **Sparse LU dengan Markowitz pivoting**.
* Solver nonlinear **Newton-Raphson**.
* Solver transient adaptif dengan kendali langkah **Local Truncation Error (LTE)**.

---

### F. `models/electrical` (`tw-electrical`) — Domain Kelistrikan & CMOS
Model domain konkret yang mengubah skema sirkuit menjadi sistem matriks MNA (*Modified Nodal Analysis*) dan menyelesaikannya menggunakan `tw-num`.

* Netlist: Resistor, Kapasitor, Sumber Tegangan/Arus independen, Dioda Shockley, dan MOSFET level 1 (Shichman-Hodges NMOS/PMOS).
* Subcircuit modular yang dapat di-nesting (misal membuat Full Adder dari gerbang CMOS).
* **Multi-Fidelity Layer**:
  - `Characterizer`: Mengukur transient delay ($t_{\text{pHL}}, t_{\text{pLH}}$), rise/fall time, dan energi switching.
  - `DerivedGate` / `DerivedDff`: Komponen diskrit `tw-core` yang perilakunya diturunkan langsung dari pengukuran transistor.
  - `CircuitComponent` (Bridge): Jembatan sinyal diskrit ke solver analog transient.
  - `FidelityFactory`: Memilih fidelity (`Transistor`, `Bridge`, `Derived`) pada waktu konstruksi.
