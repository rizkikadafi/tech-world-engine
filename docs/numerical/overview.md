# Numerical Foundation (`tw-num`) — Overview

Dokumen ini adalah **peta besar** fondasi numerik proyek (Phase 6), sebelum kita
masuk ke masing-masing teori secara mendalam. Tujuannya: menetapkan **untuk apa**
kernel numerik ini dibangun dan **dasar teori** apa yang dipakai, dengan
**ngspice** sebagai acuan implementasi kelas SPICE.

Referensi studi utama: `~/Projects/ngspice-47` (`src/maths/`, `src/spicelib/analysis/`).

> Status: **overview**. Dokumen turunan per-teori belum ditulis; lihat
> §8 untuk rencananya.

---

## Ringkasan (TL;DR)

- `tw-num` adalah **kernel numerik untuk simulator kelas SPICE**, bukan library
  matematika umum.
- Tugasnya: (1) selesaikan sistem **linear sparse** `A x = b`,
  (2) selesaikan sistem **nonlinear** `F(x) = 0`, (3) **integrasikan DAE**
  `d/dt q(x) + f(x) = b(t)` dengan **langkah adaptif**.
- **Batas keras:** `tw-num` tidak tahu apa itu resistor/node/transistor/MNA.
  Semantik rangkaian hidup di `models/electrical` (Phase 7–8).
- Dasar teorinya: aljabar linear sparse (LU + Markowitz), Newton-Raphson,
  integrasi numerik untuk DAE (Trapezoidal & BDF), dan kendali LTE/langkah.
- Dikembangkan sebagai crate baru **`tw-num`**, independen, dapat diuji tanpa
  circuit simulation (success criteria Phase 6).

---

## 1. Tujuan `tw-num` — untuk apa

`tw-num` adalah **`src/maths`-nya Tech World**: mesin matematika yang dipanggil
oleh circuit solver nanti. Ada empat tanggung jawab:

1. **Sistem linear** `A x = b` — besar, sparse, non-simetris. Ini yang dipanggil
   di *setiap* langkah (DC, tiap iterasi Newton, tiap timestep transient, AC).
2. **Sistem nonlinear** `F(x) = 0` — karena device (dioda, transistor) nonlinear.
3. **Integrasi DAE terhadap waktu** — "transient analysis", dengan langkah
   adaptif.
4. **Kendali galat & langkah** (LTE, accept/reject) dan **prediktor**.

Yang **bukan** tugas `tw-num` (lihat §2):

```text
bukan:  MNA assembly, device stamp, companion model device,
        konsep node/branch/ground, satuan volt/ampere
```

## 2. Batas domain (invarian)

```text
                     Phase 8 circuit solver
                    ┌───────────────────────┐
   device stamps ──►│  MNA:  A, b           │   ← models/electrical
                    ├───────────────────────┤
   F(x), J(x) ─────►│  Newton-Raphson loop  │   ← tw-num::nonlinear
                    ├───────────────────────┤
   riwayat langkah ►│  integrasi + LTE      │   ← tw-num::ode
                    └───────────────────────┘
                                 │
                        tw-num::linear  (LU sparse/dense)
```

Aturan: **`tw-num` index-agnostic dan domain-agnostic.** Konvensi node/ground,
penomoran baris-kolom MNA, dan satuan adalah urusan `models/electrical`.

Ini melanjutkan invarian proyek: `tw-core` tetap murni, dan `tw-num` berdiri
sendiri.

```text
tw-cli ─► tw-sim ─► tw-core
                       ▲
models/electrical ─────┴──► tw-num     (tw-num tidak bergantung pada engine)
```

## 3. Peta ngspice → `tw-num`

Setiap bagian numerik ngspice punya teori dan calon modul `tw-num`:

| ngspice | Teori / dasar | Modul `tw-num` |
| --- | --- | --- |
| `maths/sparse/spfactor.c`, `spsolve.c` | faktorisasi **LU** + pivot **Markowitz** (Sparse 1.4, Kundert) | `linear::sparse::lu` |
| `maths/dense/dense.c` | LU dense, partial pivoting | `linear::dense::lu` |
| `maths/KLU/` (SuiteSparse) | BTF + AMD + COLAMD, LU | (opsional, nanti) |
| `ni/niiter.c` | **Newton-Raphson** | `nonlinear::newton` |
| `ni/niconv.c` | konvergensi `reltol·max(|old|,|new|)+abstol` | `nonlinear::convergence` |
| `ni/niinteg.c` | **Trapezoidal & Gear (BDF)** + koefisien (`CKTag`) | `ode::methods` |
| `ni/nipred.c` | prediktor polinomial | `ode::predictor` |
| `ckttrunc.c` | estimasi **LTE** & usul langkah baru | `ode::step_control` |
| `dctran.c` | loop transient, breakpoint, accept/reject step | `ode::transient` (driver) |
| `ni/niaciter.c` | solve linear **kompleks** (AC) | `linear::complex` |
| `ni/nipzmeth.c` | pole-zero (QZ) | (nanti, opsional) |

## 4. Dasar teori inti

### a. Aljabar linear sparse — jantung SPICE

- Sistem berasal dari **MNA** (Modified Nodal Analysis, Ho–Ruehli–Brennan 1975):
  `A` **sparse, tak simetris, sering struktural-simetris**.
- Metode utama: **LU langsung** dengan **pivot Markowitz** untuk meminimalkan
  *fill-in*.
- Nuansa penting: ngspice memisahkan **analisis simbolik (ordering)** dari
  **faktorisasi numerik**, dan **memakai ulang ordering** antar-timestep
  (`SMPreorder`, flag `NIACSHOULDREORDER`) karena pola `A` jarang berubah.
- Iteratif (CG/GMRES) **bukan** jalur utama; simpan sebagai opsi khusus.

### b. Sistem nonlinear — Newton-Raphson

- Selesaikan `F(x) = 0`, dengan `F` = residu KCL/KVL + persamaan device.
- Iterasi `x_{k+1} = x_k − J(x_k)⁻¹F(x_k)`; `J` = matriks konduktansi/companion
  (yang "di-load" tiap iterasi).
- **Konvergensi berlapis** (ngspice):
  - node tegangan: `reltol·max(|old|,|new|) + voltTol`
  - cabang arus: `reltol·max(...) + abstol`
  - cek per-device (`NEWCONV` → `CKTconvTest`)

### c. Integrasi numerik (transient) — sistem **DAE**, bukan sekadar ODE

- MNA menghasilkan **DAE index-1**: `d/dt q(x) + f(x) = b(t)`. Metode implisit
  diterapkan langsung ke DAE → lahirnya **companion model**.
- **Trapezoidal**: orde 2, A-stable, tapi bisa *ringing*.
- **Gear/BDF** orde 1–6: untuk sistem *stiff*; ngspice pakai `CKTorder` dinamis
  dan **tidak menaikkan orde dari Backward Euler** tanpa alasan
  (`dctran.c` baris 821–833).
- **Koefisien bergantung rasio langkah** (variable-step) dihitung di `NIcomCof`
  → `CKTag`. Di `tw-num` ini fungsi murni:
  `coeffs(history, h, order, method) -> α/β`.

### d. Local Truncation Error & kendali langkah

- LTE diestimasi dari selisih **prediktor vs korektor**, diskalakan ke toleransi
  dan orde (`ckttrunc.c`):
  - orde 1: `Δbaru ≈ Δ·|tol·2/diff|`
  - orde 2: `… sqrt(...)`
  - orde p: `… (·)^(1/(p+1))`
- Aturan: terima bila `Δbaru > 0.9·Δ`; jika tidak **tolak & ulangi** dengan `Δ`
  lebih kecil; ada `delmin` → error `Timestep too small`; ada **breakpoint**
  (sumber/event) yang memaksa langkah.

### e. Prediktor

- Ekstrapolasi polinomial dari titik lampau; dipakai untuk **tebakan awal NR**
  sekaligus **estimasi LTE**.

### f. Strategi konvergensi sulit (desain harus siap)

- **gmin stepping**, **source stepping**, **pseudo-transient** — wajib untuk
  circuit transistor nyata. Sediakan hook di `nonlinear` sejak awal; implementasi
  belakangan.

### g. Determinisme (khas proyek kita)

- Konvergensi berbasis toleransi + iterasi berbatas → hasil `f64` bisa berbeda
  antar-platform.
- Kebijakan `tw-num`: **tanpa fast-math**, urutan operasi reduksi tetap (sparse
  ops deterministik), aturan **tie-break pivot** eksplisit, kriteria konvergensi
  numerik (bukan `==`).

## 5. Urutan teori → implementasi

| Batch | Fokus teori | Deliverable | Uji |
| --- | --- | --- | --- |
| **N1** | LU dense + partial pivoting | `Vector`, `Matrix`, `lu_solve` | sistem 2×2/3×3 vs solusi eksak |
| **N2** | Sparse + *fill-in* | `SparseMatrix` (CSR/CCS), **sparse LU Markowitz**, split simbolik/numerik | Poisson 1D; ukur fill-in |
| **N3** | Newton-Raphson | `newton(F, J, x0, tol)` + konvergensi berlapis | konvergensi kuadratik; `Singular`/`NoConvergence` |
| **N4** | DAE integration + LTE | BE/Trap/BDF + prediktor + step control | RC / RLC vs solusi analitik |
| **N5** | kompleks & strategi sulit (Phase 8) | AC complex solve, gmin/source stepping | rangkaian AC sederhana |

## 6. Keputusan yang perlu disepakati

1. **Dense dulu, sparse sesudahnya?** Saran: N1 dense sebagai *testbed*, lalu N2
   sparse sebagai tulang punggung.
2. **Direct LU vs iteratif?** Saran: **direct sparse LU** (gaya SPICE).
3. **Ordering pivot**: Markowitz (kanonik SPICE) vs AMD/COLAMD (gaya KLU).
   Saran: mulai **Markowitz**, ukur fill-in, baru pertimbangkan AMD.
4. **Representasi**: real `f64` dulu; kompleks disiapkan sebagai generik sejak N1
   agar AC tidak mahal menyusul.
5. **Determinisme**: tetapkan kebijakan tie-break & kriteria konvergensi sejak
   awal (lihat §4.g).

## 7. Alur pemakaian nanti

```text
Phase 8 circuit solver
   ├── [load stamps] ─► A, b                    (models/electrical)
   │      └── tw-num::linear::solve             ← LU sparse
   ├── Newton-Raphson loop (F, J)               ← tw-num::nonlinear
   │      └── tw-num::integrate (BE/Trap/BDF)   ← tw-num::ode
   └── tw-num::step_control (LTE)               ← tw-num::ode
```

## 8. Rencana sub-dokumen (eksplorasi mendalam)

Folder `docs/numerical/` akan menampung dokumen per-teori:

- `overview.md` — **dokumen ini** (peta besar).
- `linear-algebra.md` — LU, partial pivoting, sparse (CSR/CCS), fill-in,
  Markowitz/AMD, analisis simbolik vs numerik.
- `nonlinear.md` — Newton-Raphson, Jacobian, konvergensi berlapis, gmin/source
  stepping, pseudo-transient.
- `integration.md` — DAE, Trapezoidal, Gear/BDF, koefisien variable-step,
  prediktor, LTE & kendali langkah.
- `determinism.md` — kebijakan reproduktibilitas untuk komputasi `f64`.

## 9. Referensi

Implementasi acuan (studi):

- `~/Projects/ngspice-47/src/maths/ni/` — `niiter.c`, `niconv.c`, `niinteg.c`,
  `nipred.c`.
- `~/Projects/ngspice-47/src/maths/sparse/` — `spfactor.c`, `spsolve.c`
  (Sparse 1.4 / Kundert).
- `~/Projects/ngspice-47/src/spicelib/analysis/` — `dctran.c`, `ckttrunc.c`.

Literatur kanonik:

- Nagel, *SPICE2: A Computer Program to Simulate Semiconductor Circuits* (1975).
- Quarles, *Analysis of Performance and Convergence Issues for Circuit
  Simulation* (SPICE3, 1989).
- Ho, Ruehli, Brennan, *The Modified Nodal Approach to Network Analysis* (1975).
- Kundert & Sangiovanni-Vincentelli, *Sparse Matrix Techniques* / Sparse 1.4.
- Vlach & Singhal, *Computer Methods for Circuit Analysis and Design*.
- Gear, *Numerical Initial Value Problems in Ordinary Differential Equations*.
- Hairer & Wanner, *Solving Ordinary Differential Equations* I/II.
- Duff, Erisman, Reid, *Direct Methods for Sparse Matrices*.
- Davis, *Direct Methods for Sparse Linear Systems*.
- Najm, *Circuit Simulation*.
- Pillage, Rohrer, Visweswariah, *Electronic Circuit and System Simulation
  Methods*.

Dokumen internal terkait:

- `engine-v1.md` — Phase 6–8 (Numerical → Circuit Representation → Circuit
  Solver).
- `time-model.md` — sinkronisasi waktu diskret vs kontinu (relevan Phase 7–8).
- `implementation.md` — status fondasi P0–P2.
