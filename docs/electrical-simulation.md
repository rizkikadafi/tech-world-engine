# Domain Rangkaian Listrik & Transistor-Level MNA Solver (`tw-electrical`)

Dokumen ini menjelaskan **implementasi domain rangkaian listrik (Phase 7)**, **solver rangkaian MNA (Phase 8)**, **device non-linear (Phase 9)**, dan **simulasi level transistor (Phase 10)** yang berada di [`models/electrical/`](file:///home/rkcode/Projects/tech-world-engine/models/electrical) (package `tw-electrical`).

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

Jalankan seluruh pengujian dengan perintah:
```bash
cargo test -p tw-electrical
```

---

## 6. Roadmap Lanjutan (Next Steps)

1. **Subcircuits & Hierarchical Logic Gates (NAND, NOR, D-Flip-Flop)**:
   - Membangun gerbang logika berstruktur hierarkis dari MOSFET dan resistor.
2. **Adaptive Timestep & LTE Control (Phase 6 / N4)**:
   - Memilih $\Delta t$ secara dinamis berdasarkan laju perubahan tegangan (*Local Truncation Error*).
3. **Sparse Matrix Formulation (Phase 6 / N2)**:
   - Mengalihkan backend `tw-electrical` dari dense matrix ke sparse matrix (`CSR`/`CSC` + Markowitz LU) untuk rangkaian dengan ratusan/ribuan gerbang logika.
