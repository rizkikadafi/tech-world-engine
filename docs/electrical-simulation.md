# Domain Rangkaian Listrik & CMOS Logic Gates Solver (`tw-electrical`)

Dokumen ini menjelaskan **implementasi domain rangkaian listrik (Phase 7)**, **solver rangkaian MNA (Phase 8)**, **device non-linear (Phase 9)**, **simulasi level transistor (Phase 10)**, dan **gerbang logika CMOS & subcircuit modular (Phase 11 — Milestone Besar #1)** yang berada di [`models/electrical/`](file:///home/rkcode/Projects/tech-world-engine/models/electrical) (package `tw-electrical`).

---

## 1. Posisi dalam Arsitektur Engine

Prinsip dasar dari Tech World Engine adalah **pemisahan keras (*strict decoupling*)**:
- [`tw-core`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/): Domain-agnostic primitives (Time, State, Event, Component, Resource, Causal Tracing).
- [`tw-num`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-num/): Domain-agnostic numerical mathematics (Dense LU factorization, Matrix, Vector, ODE). Tidak tahu apa itu resistor, volt, atau node.
- [`tw-electrical`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/): **Domain model konkret**. Mengubah graf rangkaian listrik menjadi sistem persamaan linear $A x = b$ (MNA), lalu memanggil `tw-num` untuk menyelesaikannya.

```text
                  tw-cli
                    │
                    ▼
                  tw-sim
                    │
         ┌──────────┴──────────┐
         ▼                     ▼
      tw-core            models/electrical (tw-electrical)
                               │
                               ▼
                            tw-num (Dense LU Solver)
```

Arah dependensi tetap satu arah:
`models/electrical` bergantung pada `tw-core` dan `tw-num`. Sebaliknya, `tw-core` dan `tw-num` tidak memiliki dependensi balik ke `models/electrical`.

---

## 2. Model & Representasi Sirkuit (Phase 7)

Rangkaian direpresentasikan sebagai graf berarah dengan node dan cabang (*branches*).

### A. Node & Ground Convention
- Setiap node diidentifikasi oleh [`NodeId`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/src/node.rs).
- **Node 0 (`NodeId::GROUND`)** adalah *reference ground* bertegangan mutlak $0\text{ V}$.
- Node ground tidak dimasukkan sebagai variabel bebas dalam matriks MNA, sehingga mengurangi dimensi sistem sebesar 1 dan mencegah matriks menjadi singular.
- String `"0"`, `"GND"`, dan `"gnd"` otomatis dipetakan ke `NodeId::GROUND`.

### B. Device Model
Tersedia 6 elemen sirkuit dasar di [`models/electrical/src/device.rs`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/src/device.rs):
1. **Resistor ($R$)**: Hubungan linier $V = I \cdot R$ dengan konduktansi $G = 1/R$.
2. **Capacitor ($C$)**: Hubungan diferensial $I = C \frac{dV}{dt}$.
3. **Independent Voltage Source ($V_{src}$)**: Menetapkan beda potensial ideal $V_{pos} - V_{neg} = V_{src}$.
4. **Independent Current Source ($I_{src}$)**: Mengalirkan arus konstan dari terminal negatif ke terminal positif.
5. **Diode ($D$)**: Model Shockley $I_D = I_S \left(e^{V_D / (\eta V_T)} - 1\right)$ dengan ekstrapolasi linier untuk mencegah overflow floating-point.
6. **MOSFET (NMOS & PMOS Level 1)**: Model Shichman-Hodges dengan 3 daerah kerja (Cutoff, Triode/Linear, Saturation) dan modulasi panjang kanal $\lambda$.

### C. Circuit Netlist Builder
[`Circuit`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/src/circuit.rs) menyediakan builder API untuk merangkai komponen:
```rust
let mut ckt = Circuit::new();
let n_in = ckt.get_or_add_node("in");
let n_out = ckt.get_or_add_node("out");

ckt.add_voltage_source("V1", n_in, NodeId::GROUND, 5.0);
ckt.add_resistor("R1", n_in, n_out, 1000.0);
ckt.add_capacitor("C1", n_out, NodeId::GROUND, 1e-6);
```

---

## 3. Solver MNA & Stamping Otomatis (Phase 8)

MNA (*Modified Nodal Analysis*) adalah standar industri (digunakan oleh SPICE dan ngspice) untuk mengubah skema rangkaian menjadi persamaan aljabar matriks:

$$
\begin{bmatrix}
G & B \\
C & D
\end{bmatrix}
\begin{bmatrix}
v \\
j
\end{bmatrix}
=
\begin{bmatrix}
i \\
e
\end{bmatrix}
$$

Dimensi total matriks adalah $(N + M) \times (N + M)$:
- $N$: Jumlah node non-ground.
- $M$: Jumlah sumber tegangan independen (arus cabang $j$ menjadi variabel yang dicari).

### A. Aturan Stamping

| Elemen | Persamaan Dasar | Modifikasi Matriks MNA | Modifikasi Vektor RHS |
| --- | --- | --- | --- |
| **Resistor** $R$ ($n_+, n_-$) | $I = G(V_+ - V_-)$ | $G[n_+, n_+] \mathrel{+}= g$<br/>$G[n_-, n_-] \mathrel{+}= g$<br/>$G[n_+, n_-] \mathrel{-}= g$<br/>$G[n_-, n_+] \mathrel{-}= g$ | *(tidak ada)* |
| **Current Source** $I_{src}$ ($n_- \to n_+$) | Arus masuk ke $n_+$ | *(tidak ada)* | $i[n_+] \mathrel{+}= I_{src}$<br/>$i[n_-] \mathrel{-}= I_{src}$ |
| **Voltage Source** $V_{src}$ cabang $k$ | $V_+ - V_- = V_{src}$ | $B[n_+, k] \mathrel{+}= 1, B[n_-, k] \mathrel{-}= 1$<br/>$C[k, n_+] \mathrel{+}= 1, C[k, n_-] \mathrel{-}= 1$ | $e[k] = V_{src}$ |

*(Catatan: Operasi pada node ground/0 diabaikan).*

### B. Analisis Transient: Companion Model Kapasitor (Backward Euler)
Kapasitor memiliki relasi diferensial waktu:

$$
I_C(t) = C \frac{dV(t)}{dt}
$$

Dengan diskritisasi implisit **Backward Euler** langkah waktu $\Delta t$:

$$
I_C(t_{n+1}) = C \frac{V(t_{n+1}) - V(t_n)}{\Delta t} = \frac{C}{\Delta t} V(t_{n+1}) - \frac{C}{\Delta t} V(t_n)
$$

Bentuk ini dimodelkan sebagai **rangkaian ekuivalen (*companion model*)**:
- Resistor dengan konduktansi ekuivalen:
  $$G_{eq} = \frac{C}{\Delta t}$$
  di-stamp ke submatriks $G$ antara node $n_+$ dan $n_-$.
- Sumber arus riwayat independen:
  $$I_{eq} = G_{eq} \cdot \left(V_{pos}(t_n) - V_{neg}(t_n)\right)$$
  mengalir dari $n_-$ ke $n_+$, sehingga di-stamp ke vektor RHS $i$:
  $$i[n_+] \mathrel{+}= I_{eq},\quad i[n_-] \mathrel{-}= I_{eq}$$

Dengan pendekatan ini, persamaan diferensial kapasitor terkonversi menjadi sistem linear biasa pada setiap langkah waktu!

---

## 4. Mode Analisis yang Didukung

### 1. DC Operating Point (`dc_op`)
Digunakan untuk mencari tegangan dan arus kondisi mantap (*steady-state*) DC:
- Seluruh kapasitor dianggap *open circuit* ($I_C = 0$).
- Membentuk matriks sistem DC dan memanggil `tw_num::linear::solve`.
- Mengembalikan [`CircuitState`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/src/state.rs) berisi tegangan tiap node dan arus pada tiap sumber tegangan.

```rust
let state = dc_op(&circuit)?;
let v_out = state.voltage(n_out);
let i_source = state.current("V1").unwrap();
```

### 2. Transient Analysis (`transient`)
Digunakan untuk mensimulasikan dinamika rangkaian terhadap waktu:
- Menerima parameter `t_stop`, langkah waktu `dt`, dan initial state opsional.
- Pada setiap step $t_{k+1} = t_k + \Delta t$, MNA assembler menyusun konduktansi dan sumber arus riwayat kapasitor berdasarkan tegangan step sebelumnya.
- Mengembalikan [`TransientResult`](file:///home/rkcode/Projects/tech-world-engine/models/electrical/src/analysis.rs) yang memuat riwayat state di setiap titik waktu dan menyediakan helper `voltage_waveform(node)`.

```rust
let result = transient(&circuit, 0.005, 1e-5, None)?;
let waveform = result.voltage_waveform(n_out); // Vec<(t, V)>
```

---

## 5. Verifikasi dan Pengujian

Pengujian integrasi otomatis berada di direktori `models/electrical/tests/`:

### A. Jaringan Resistor (`tests/resistor_networks.rs`)
1. **Hukum Ohm Dasar**: Menguji sumber 10V dengan resistor 50Ω. Memverifikasi $V = 10\text{ V}$ dan arus cabang $I = -0.2\text{ A}$ (konvensi MNA arus keluar sumber).
2. **Voltage Divider**: Memverifikasi pembagian tegangan pada node tengah:
   $$V_{mid} = V_{DD} \frac{R_2}{R_1 + R_2}$$
3. **Three-Resistor Ladder**: Memverifikasi rangkaian 3 resistor bertingkat.
4. **Wheatstone Bridge**: Memverifikasi jembatan Wheatstone dengan kondisi seimbang $R_1 / R_2 = R_3 / R_4$, membuktikan tegangan node jembatan identik ($V_B = V_C$) dan arus resistor jembatan bernilai nol.
5. **Current Source Paralel**: Memverifikasi injeksi arus independen ke node resistif ($V = I \cdot R$).

### B. Dinamika Kapasitor & RC (`tests/rc_circuits.rs`)
1. **RC Step Charging**:
   - Diberi step tegangan $V_{DD}$ pada $t=0$.
   - Hasil numerik dibandingkan dengan solusi analitik fisika:
     $$V(t) = V_{DD}\left(1 - e^{-t / RC}\right)$$
   - Diverifikasi pada beberapa titik waktu ($0.5\tau, 1.0\tau, 2.0\tau, 3.0\tau, 5.0\tau$) dengan galat relatif $< 1\%$.
2. **RC Discharging**:
   - Kapasitor diberi tegangan awal $V_0 = 10\text{ V}$ lalu dilepas ke resistor tanpa sumber.
   - Hasil dibandingkan dengan solusi analitik $V(t) = V_0 e^{-t / RC}$ dengan galat relatif $< 1\%$.

### C. Rangkaian Dioda (`tests/diode_circuits.rs`)
1. **Diode Forward Bias**: Menguji dioda silikon dengan resistor $1\text{ k}\Omega$ pada $5\text{ V}$. Dioda terkonvergensi pada penurunan tegangan $0.65\text{ V} - 0.75\text{ V}$ dan memenuhi KCL hukum Shockley dengan galat $< 2\%$.
2. **Diode Reverse Bias**: Menguji tegangan mundur $-5\text{ V}$. Dioda memblokir arus sehingga arus mendekati 0 dan tegangan katoda mendekati $-5\text{ V}$.

### D. Rangkaian MOSFET & CMOS Inverter (`tests/mosfet_circuits.rs`)
1. **NMOS Resistive Inverter**: Sakelar NMOS dengan pull-up $10\text{ k}\Omega$. Ketika $V_{in} = 0\text{ V}$ (Cutoff), $V_{out} = 5.0\text{ V}$. Ketika $V_{in} = 5.0\text{ V}$ (Triode), $V_{out} < 0.2\text{ V}$.
2. **CMOS Inverter Logic Levels**: Pasangan komplementer NMOS dan PMOS:
   - $V_{in} = 0.0\text{ V}$ (Logic LOW) $\implies V_{out} \approx 5.0\text{ V}$ (Logic HIGH).
   - $V_{in} = 5.0\text{ V}$ (Logic HIGH) $\implies V_{out} \approx 0.0\text{ V}$ (Logic LOW).
   - $V_{in} = 2.5\text{ V}$ (Threshold tengah) $\implies V_{out} \approx 2.5\text{ V}$.
3. **CMOS Transient Discharge**: Kapasitor beban $10\text{ pF}$ terisi $5\text{ V}$ lalu di-discharge oleh NMOS saat input switched HIGH. Tegangan turun secara monotonik ke $< 0.1\text{ V}$.

### E. Gerbang Logika CMOS & Subcircuits (`tests/cmos_gates.rs`)
1. **CMOS NAND Gate**: 2 PMOS paralel antara VDD dan OUT, 2 NMOS seri antara OUT dan GND.
   - Terverifikasi pada 4 kombinasi tabel kebenaran ($00 \to 1, 01 \to 1, 10 \to 1, 11 \to 0$) dengan level logika mantap ($> 4.8\text{V}$ dan $< 0.2\text{V}$).
2. **CMOS NOR Gate**: 2 PMOS seri antara VDD dan OUT, 2 NMOS paralel antara OUT dan GND.
   - Terverifikasi pada 4 kombinasi tabel kebenaran ($00 \to 1, 01 \to 0, 10 \to 0, 11 \to 0$).
3. **Compound CMOS AND Gate**: Gerbang NAND dikaskade dengan inverter CMOS, terverifikasi menghasilkan fungsi logika AND.

### F. Memori Fisik Sekuensial: SR Latch (`tests/sr_latch.rs`)
1. **Bistable Memory Retention**: Dua gerbang CMOS NOR dirangkai secara *cross-coupled* dengan kapasitor simpan muatan ($1\text{ pF}$).
   - **SET Phase** ($S=5\text{V}, R=0\text{V}$): $Q$ terisi muatan hingga $> 4.5\text{V}$, $\bar{Q} < 0.5\text{V}$.
   - **HOLD Phase 1** ($S=0\text{V}, R=0\text{V}$): Input dilepas, state $Q$ tetap bertahan $> 4.5\text{V}$.
   - **RESET Phase** ($S=0\text{V}, R=5\text{V}$): $Q$ dikosongkan ke $< 0.5\text{V}$, $\bar{Q}$ terisi $> 4.5\text{V}$.
   - **HOLD Phase 2** ($S=0\text{V}, R=0\text{V}$): Input dilepas, state $Q$ tetap bertahan $< 0.5\text{V}$.
   - **Kesimpulan**: Memori 1-bit berhasil tercipta murni dari dinamika medan listrik transistor fisik tanpa percabangan logika software (`if/else`).

### G. Nested Subcircuits (`tests/subcircuit_nesting.rs`)
1. **Rekursi Template**: `Subcircuit` kini dapat berisi `SubcircuitInstance` lain. Setiap instance mendapat prefix nama sendiri, sehingga node internal tidak bertabrakan.
2. **Ekuivalensi**: AND yang dibangun dari NAND + inverter bertingkat menghasilkan tegangan yang identik dengan versi flat pada keempat kombinasi input.
3. **Isolasi Instance**: Dua instance template yang sama memiliki node internal terpisah (`U1_NAND1_n_mid` vs `U2_NAND1_n_mid`).
4. **Validasi Binding**: Port child yang tidak dikenal, port yang tidak di-bind, dan nama instance duplikat ditolak dengan `CircuitError`.

### H. Unit Kombinasional Digital (`tests/digital_combinational.rs`)
1. **CMOS XOR**: Empat gerbang NAND; tabel kebenaran $00 \to 0, 01 \to 1, 10 \to 1, 11 \to 0$.
2. **CMOS Half Adder**: XOR untuk `SUM`, AND untuk `CARRY`; 4 kombinasi terverifikasi.
3. **CMOS Full Adder**: Dua half adder + OR; seluruh 8 kombinasi $(A, B, C_{in})$ terverifikasi untuk `SUM` dan `COUT`.

### I. Unit Sekuensial Digital (`tests/digital_sequential.rs`)
1. **D Latch**: NAND-based gated latch.
   - `EN=1` transparan: `Q` mengikuti `D`.
   - `EN=0` menyimpan: perubahan `D` tidak mengubah `Q`.
2. **D Flip-Flop Master–Slave**:
   - Master transparan saat `CLK=0`, slave transparan saat `CLK=1`.
   - `Q` hanya berubah pada **rising edge**.
   - Perilaku hold terverifikasi saat `D` berubah di luar edge dan pada falling edge.
   - `Q_BAR` selalu komplemen `Q`.
3. **Parameter Simulasi**: $\Delta t = 0.1\text{ ns}$, durasi 5 ns per fase, kapasitansi simpan $1\text{ pF}$.

Jalankan seluruh pengujian dengan perintah:
```bash
cargo test -p tw-electrical
```

---

## 6. Multi-Fidelity & Mixed-Signal Bridge (Milestone Besar #2 — Slice 1–3)

Milestone ini membuktikan alur penuh untuk tiga sirkuit fisik:

```text
transistor inverter / NAND / DFF
        │ transient + dc_op
        ▼
measure t_pHL, t_pLH, t_r, t_f, energy
        │
        ▼
derived tw_core::Component
        │
        ▼
validate: truth table / next state + timing vs physical
```

Semua kode tetap berada di `tw-electrical`; `tw-core` dan `tw-sim` tidak tahu apa itu transistor, MOSFET, inverter, atau flip-flop.

### A. Time base eksplisit

`tw-core` memodelkan waktu sebagai `u64` tick tanpa satuan fisik. Modul `time_base.rs` menambahkan:

```rust
pub struct TimeBase {
    pub seconds_per_tick: f64, // default 1 ns
}
```

`TimeBase::delay_ticks(seconds)` membulatkan ke atas dan selalu menghasilkan minimal **1 tick**, sehingga komponen turunan tidak pernah menghasilkan transisi zero-delay yang melanggar kausalitas.

### B. Automated characterizer (`characterization.rs`)

`Characterizer` menyediakan tiga entry point:

- `inverter()` — CMOS inverter satu input.
- `nand2()` — NAND 2-input; edge turun diukur dengan `A=1` dan `B` naik, edge naik dengan `A=1` dan `B` turun.
- `dff()` — master-slave D flip-flop positive-edge-triggered.

Selain itu `measure(kind)` mengembalikan `Measurement { gate, template }`. `template` adalah `CircuitTemplate` berisi netlist fisik yang **persis** dipakai untuk pengukuran, port map diskrit, node output, dan nama supply. Template inilah yang dipakai ulang oleh bridge, sehingga model analog tidak pernah dibangun dari netlist yang berbeda.

Langkah umum untuk gerbang kombinasional:

1. Menjalankan `dc_op` untuk semua kombinasi input → tabel kebenaran serta level logika terukur (`LogicLevels`).
2. Menjalankan `transient` untuk satu edge turun dan satu edge naik.
3. Mencari crossing 10%, 50%, dan 90% dengan interpolasi linear antar sampel.
4. Mengintegrasikan energi supply dengan aturan trapesium:

```text
E = ∫ V_supply · (−I_supply) dt
```

Konvensi tanda mengikuti MNA: arus source bernilai negatif saat source mengalirkan daya.

DFF tidak punya tabel kebenaran kombinasional. Yang diukur adalah:

1. Inisialisasi slave latch dengan clock pulse saat `D=0`.
2. `D=1`, `CLK=0`, settle → master menangkap 1.
3. Edge naik `CLK` → ukur `t_pLH`, `t_r`, dan energi edge naik.
4. `D=0`, `CLK=0`, settle → master menangkap 0.
5. Edge naik `CLK` → ukur `t_pHL`, `t_f`, dan energi edge turun.

`settle_duration` (default 10 ns) memberi latch internal waktu untuk menetap sebelum edge clock diukur. Tabel kebenaran yang dikembalikan adalah **next-state table**: `Q` setelah edge naik sama dengan `D`, sehingga kolom `CLK` bersifat *don't-care*.

Hasil default (Vdd 5 V, CL 10 pF, Δt 0.05 ns, durasi 20 ns, β = 1e-3, Vth = 0.8 V):

| Metrik | Inverter | NAND 2-input | DFF (CLK→Q) |
| --- | ---: | ---: | ---: |
| `t_pHL` | ≈ 2.75 ns | ≈ 2.85 ns | ≈ 3.51 ns |
| `t_pLH` | ≈ 2.75 ns | ≈ 2.75 ns | ≈ 3.03 ns |
| `t_r` | ≈ 6.68 ns | ≈ 6.68 ns | ≈ 5.33 ns |
| `t_f` | ≈ 6.68 ns | ≈ 6.81 ns | ≈ 7.47 ns |
| `energy_rise` | ≈ 249.9 pJ | ≈ 249.9 pJ | ≈ 286.1 pJ |
| `energy_fall` | ≈ 0 pJ | ≈ 0 pJ | ≈ 25.1 pJ |
| `energy_per_cycle` | ≈ 249.9 pJ | ≈ 249.9 pJ | ≈ 311.2 pJ |

Angka energi rise inverter/NAND mendekati $C_L V_{dd}^2 = 10\text{ pF} \times 25\text{ V}^2 = 250\text{ pJ}$, yang menjadi sanity check fisik. DFF lebih tinggi karena ikut mengisi node internal latch dan clock inverter.

### C. Derived component (`derived.rs`)

Ada dua komponen turunan, keduanya mengubah hasil ukur menjadi `tw_core::Component`:

- `DerivedGate` — inverter dan NAND 2-input.
- `DerivedDff` — D flip-flop positive-edge-triggered.

Aturan bersama:

- **Logika berasal dari pengukuran fisik**, bukan `match` fungsi gerbang. Untuk gerbang kombinasional tabel kebenaran diambil dari `dc_op`; untuk DFF tabel next-state diambil dari edge clock yang benar-benar disimulasikan (`Q = D` setelah edge naik).
- Delay naik memakai `t_pLH`, delay turun memakai `t_pHL`, masing-masing dikuantisasi ke tick.
- Event output memakai `EventPayload::Custom` internal + **generation counter** (inertial delay), sehingga edge yang sudah basi dibatalkan bila input berubah lebih cepat dari delay.
- `State` mengekspos `output`, `pending`, `delay_ticks`, `t_pHL_ticks`, `t_pLH_ticks`, serta metrik detik/energi hasil pengukuran.

`DerivedDff` menambahkan semantik edge-triggered:

- `D` hanya disampling pada transisi `CLK` rendah → tinggi.
- Perubahan `D` saat `CLK` tinggi atau rendah tidak langsung mengubah `Q`.
- Bila edge clock baru menghasilkan nilai yang sama dengan `Q` yang sudah commit, edge pending dibatalkan.
- Port: `D` (1), `CLK` (2), `Q` (3).

### D. Mixed-signal bridge (`bridge.rs`)

`CircuitComponent` adalah adapter `tw_core::Component`:

- `BridgeInput` memetakan `PortSignal` diskrit ke voltage source di dalam `Circuit`.
- `BridgeOutput` memetakan node analog ke `PortSignal` diskrit melalui threshold.
- Setiap `sync_interval_ticks`, bridge menjalankan `transient` sepanjang interval tersebut, menyimpan `CircuitState` terakhir, dan hanya mengeluarkan event bila logika output berubah.
- Saat input berubah, bridge lebih dulu mengintegrasikan waktu yang sudah berlalu dengan nilai input lama, lalu menerapkan nilai baru. Tidak ada waktu fisik yang hilang atau dihitung dua kali.
- Energi supply diakumulasi di `State` (`energy_joules`).
- Kegagalan solver tidak membuat panic: error dicatat di `last_error`, output terakhir dipertahankan.
- Bila `dc_op` awal gagal karena sirkuit bistabil (misalnya master-slave DFF dengan latch cross-coupled), bridge menjalankan startup transient 10 ns dari state nol untuk menetapkan state analog awal secara deterministik.
- `BridgeConfig` memvalidasi bahwa semua `source_name` input dan `supply_source` benar-benar ada di `Circuit`, sehingga salah ketik gagal saat konstruksi, bukan saat simulasi.

Contoh menjalankan inverter, NAND 2-input, dan DFF sekaligus:

```bash
cargo run -p tw-electrical --example multi_fidelity
```

Output yang diharapkan (1 ns/tick):

```text
=== inverter: same discrete input applied (0 → 1) ===
bridge  output transitions: [(0, true), (3, false)]
derived output transitions: [(0, true), (3, false)]

=== NAND2: A high, B pulses high then low ===
bridge  output transitions: [(0, true), (8, false), (18, true)]
derived output transitions: [(0, true), (8, false), (18, true)]

=== DFF: capture 1, then capture 0 on rising edges ===
bridge  output transitions: [(0, false), (22, true), (47, false)]
derived output transitions: [(0, false), (22, true), (47, false)]
```

### E. Fidelity selection & refinement (`fidelity.rs`)

`fidelity.rs` menyatukan ketiga representasi di belakang satu deskripsi komponen:

```rust
pub enum Fidelity {
    Transistor, // netlist mentah
    Bridge,     // CircuitComponent (mixed-signal)
    Derived,    // DerivedGate / DerivedDff (diskrit)
}

pub struct ComponentSpec {
    pub id: ComponentId,
    pub name: String,
    pub kind: GateKind,
}
```

`FidelityFactory` menyimpan `Characterizer` dan cache in-memory per `GateKind`:

```rust
let mut factory = FidelityFactory::with_defaults();
let spec = ComponentSpec::new(ComponentId(1), "INV1", GateKind::Inverter);

let derived = factory.select_component(&spec, Fidelity::Derived)?;
let bridge  = factory.select_component(&spec, Fidelity::Bridge)?;
let netlist = factory.select(&spec, Fidelity::Transistor)?;
```

Jaminan penting:

- **Port identik.** Derived dan bridge memakai port map yang sama: inverter `IN=1, OUT=2`; NAND `A=1, B=2, OUT=3`; DFF `D=1, CLK=2, Q=3`.
- **Satu netlist.** `select(Transistor)` mengembalikan `template.circuit.clone()`, dan bridge membungkus netlist yang sama — bukan hasil `build_*` terpisah.
- **Satu pengukuran.** Hasil karakterisasi di-cache per `GateKind`. DFF yang butuh beberapa detik hanya diukur sekali per factory; `is_cached()`/`cached_count()` mengekspos status cache.
- **Threshold terukur.** Ambang output bridge memakai `levels.threshold(mid_threshold)` hasil ukur, bukan angka hardcoded.
- **Refinement monoton.** `refine()` bergerak `Derived → Bridge → Transistor`; meminta detail dari `Transistor` menghasilkan `FidelityError::AlreadyMostDetailed`.
- **Batas yang jelas.** `select_component(Transistor)` menghasilkan `FidelityError::NotAComponent`; bungkus dengan `Fidelity::Bridge` untuk menjalankannya di `tw-sim`.

Pemilihan dilakukan saat **konstruksi**. Mengganti fidelity di tengah simulasi (live switch) belum didukung; cache juga masih in-memory, belum persisten antar-proses.

Contoh:

```bash
cargo run -p tw-electrical --example fidelity_selection
```

### F. Batas implementasi saat ini

- Inverter, NAND 2-input, dan DFF sudah end-to-end. Gerbang lain (NOR, XOR, AND, OR) belum punya entry point characterizer/derived.
- Stimulus berupa step ideal; slope input berhingga belum dimodelkan.
- Setup/hold DFF belum divalidasi eksplisit; karakterizer memakai window settle yang cukup lebar, bukan batas timing formal.
- Resolusi timing bridge dibatasi `sync_interval_ticks`; glitch yang lebih pendek dari satu interval tidak terlihat.
- `solver_dt` harus lebih kecil dari durasi satu interval; bridge memilih `min(solver_dt, elapsed)` agar tetap valid.
- State analog internal bridge belum ikut checkpoint/restore penuh; hanya nilai bermakna yang disimpan di `State`.
- Bridge dengan sirkuit bistabil memakai startup transient, bukan analisis DC dengan initial condition eksplisit.

### G. Test yang membuktikan alur

- `tests/characterization.rs` — metrik inverter/NAND/DFF masuk akal, konvergensi saat Δt dibagi dua, truth table cocok dengan `dc_op`, dan sanity check tanda energi.
- `tests/derived_components.rs` — delay sesuai hasil ukur, pembatalan event basi, state metrik terukur, `DerivedDff` hanya menangkap pada edge naik.
- `tests/mixed_signal_bridge.rs` — output bridge cocok dengan transient standalone, akumulasi energi, validasi konfigurasi, perilaku NAND dan DFF.
- `tests/multi_fidelity.rs` — `detailed → measure → abstract → validate` untuk inverter, NAND 2-input, dan DFF, termasuk perbandingan tick transisi bridge vs derived (toleransi 1 tick).
- `tests/fidelity.rs` — port identik antar-fidelity untuk ketiga `GateKind`, cache karakterisasi benar-benar dipakai ulang, urutan `refine`, netlist transistor identik dengan template pengukuran, dan paritas inverter derived vs bridge end-to-end.

---

## 7. Roadmap Lanjutan (Next Steps)

1. **Larger Digital Units (Register, ALU Slice)**:
   - Menyusun register dari beberapa `cmos_dff`, lalu unit aritmatika dari full adder.
2. **Benchmark Fidelity (Milestone Besar #3)**:
   - Membandingkan biaya komputasi 1 NAND transistor vs 1.000–1.000.000 `DerivedGate`.
3. **Multi-Fidelity Lanjutan**:
   - Slice 1–3 (inverter, NAND 2-input, DFF) dan lapisan `fidelity.rs` (selection, `refine`, cache) sudah selesai.
   - Selanjutnya: gerbang lain (NOR/XOR/AND/OR), input slope berhingga, setup/hold formal, live fidelity switch, dan cache persisten.

Catatan: sparse LU + Markowitz sudah dipakai `MnaSystem`, dan `transient_adaptive` (LTE) sudah tersedia. Keduanya bukan lagi item roadmap.

---

## 8. Benchmark Fidelity (Milestone Besar #3)

Benchmark `models/electrical/benches/fidelity_scaling.rs` mengukur biaya **satu switching event NAND** pada tiga fidelity. Netlist dan hasil karakterisasi NAND2 dibuat sekali di luar region terukur, sehingga yang dibandingkan adalah biaya switching, bukan biaya pengukuran.

Semua kasus menyimulasikan window yang sama: **20 ns** (20 tick pada `TimeBase` default). Transistor menjalankan transient 20 ns; bridge menjalankan `CircuitComponent` sampai 20 tick; derived gate dipicu lalu event loop berhenti pada horizon yang sama.

### Cara menjalankan

```bash
cargo bench -p tw-electrical --bench fidelity_scaling -- --quick
```

### Hasil representatif

Angka berikut berasal dari `--quick` pada mesin pengembangan; nilai absolut akan berbeda, tetapi rasionya menunjukkan karakter biaya.

| Kasus | Waktu | Biaya per edge |
| --- | ---: | ---: |
| Transistor NAND, satu falling edge (20 ns) | ≈ 13.4 ms | 13.4 ms |
| Bridge NAND, satu falling edge (20 tick) | ≈ 13.3 ms | 13.3 ms |
| Derived NAND, 1 gate | ≈ 4.0 µs | 4.0 µs |
| Derived NAND, 1.000 gate | ≈ 5.9 ms | 5.9 µs |
| Derived NAND, 10.000 gate | ≈ 162 ms | 16.2 µs |
| Derived NAND, 100.000 gate | ≈ 1.0 s | 10.0 µs |

Konstruksi derived gate dari measurement cache:

| Jumlah | Waktu | Biaya per komponen |
| ---: | ---: | ---: |
| 1 | ≈ 8 µs | 8 µs |
| 1.000 | ≈ 7.3 ms | 7.3 µs |
| 100.000 | ≈ 0.67 s | 6.7 µs |

### Interpretasi

- Bridge dan transistor sama-sama didominasi integrasi analog: keduanya sekitar **13 ms per edge** untuk window 20 ns.
- Satu derived gate sekitar **4 µs per edge**, jadi kira-kira **3.000–3.300× lebih murah** daripada satu edge analog pada kasus satu gate.
- Pada skala 100.000 gate, biaya per gate naik menjadi sekitar **10 µs** karena event loop, `HashMap` komponen, dan tekanan cache. Meski begitu, 100.000 derived NAND yang switching masih selesai dalam sekitar **1 detik**.
- Derived gate adalah model event-driven: setelah transisi selesai, tidak ada pekerjaan tambahan sampai input berikutnya. Bridge tetap melakukan langkah analog periodik selama window simulasi.

### Stress test 1 juta komponen

Criterion berhenti di 100.000 komponen karena setiap `DerivedGate` membawa `State` map dan simulation menyimpan seluruh komponen. Untuk mencoba 1 juta:

```bash
cargo test --release -p tw-electrical --test fidelity_scale -- --ignored --nocapture
```

Override jumlah komponen bila memori terbatas:

```bash
TW_SCALE_COMPONENTS=100000 cargo test --release -p tw-electrical --test fidelity_scale -- --ignored --nocapture
```

Pada 10.000 komponen, stress test mencatat sekitar **34 MiB RSS** setelah build. Ekstrapolasi linear memberi perkiraan beberapa GiB untuk 1 juta komponen, jadi hasil aktual sangat bergantung pada RAM. Test mencetak waktu karakterisasi, build + mount, switch + run, jumlah event, RSS, dan simulated-ns/detik.

### Batas benchmark

- Perbandingan memakai satu NAND2; gerbang lain dan DFF belum diukur.
- Bridge memakai `sync_interval_ticks = 1` dan `solver_dt` dari hasil karakterisasi.
- Angka wall time bergantung pada CPU, allocator, dan beban mesin.
- 1 juta komponen adalah stress test manual, bukan bagian dari CI.
