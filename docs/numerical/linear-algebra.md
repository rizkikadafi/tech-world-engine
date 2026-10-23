# Numerical Foundation — Linear Algebra

Dokumen ini adalah pendalaman pertama dari `overview.md`. Fokusnya: **teori
aljabar linear** yang menjadi tulang punggung `tw-num`, dengan **Sparse 1.4
(Kundert, dipakai ngspice)** sebagai acuan implementasi.

Sasaran dokumen: memahami *apa* yang diselesaikan, *bagaimana* secara teori, dan
*implikasi desainnya* untuk crate `tw-num`.

---

## Ringkasan (TL;DR)

- Setiap langkah simulasi pada akhirnya berarti **menyelesaikan `A x = b`**.
- `A` bersifat **besar, sparse, tak simetris, dan (hampir) simetris secara
  struktural**, biasanya **dominan diagonal**.
- Untuk matriks kecil: **LU dense + partial pivoting** cukup (dan penting sebagai
  *testbed*).
- Untuk matriks nyata: **LU sparse** dengan **pivot ber-threshold** untuk
  kestabilan **+ kriteria Markowitz** untuk meminimalkan *fill-in*.
- Kunci performa SPICE adalah pemisahan **analisis simbolik (ordering)** dari
  **faktorisasi numerik**, dan **memakai ulang ordering** antar-timestep.
- Iteratif (CG/GMRES) **bukan** jalur utama; disimpan sebagai opsi khusus.

---

## 1. Peran & konteks

Aljabar linear dipanggil di hampir semua analisis:

```text
DC operating point      ─► NR: solve A x = b tiap iterasi
Transient               ─► tiap timestep: NR → solve A x = b
AC / small-signal       ─► solve A x = b dengan A kompleks
Sensitivity (adjoint)   ─► solve Aᵀ x = b  (transposed solve)
```

Jadi performa solver linear **mendominasi** performa simulator. Inilah alasan
`tw-num` dimulai dari sini.

### 1.1 Sifat matriks

Motivasi datang dari **MNA** (Modified Nodal Analysis), tetapi `tw-num` tetap
domain-agnostic — ia hanya melihat matriks. Sifat tipikal:

| Sifat | Arti | Konsekuensi |
| --- | --- | --- |
| **Sparse** | mayoritas entri nol | wajib pakai representasi sparse |
| **Tak simetris** | `A[i][j] ≠ A[j][i]` secara nilai | butuh LU, bukan Cholesky |
| **Simetris struktural** | pola nol simetris | ordering & fill-in bisa dieksploitasi |
| **Hampir dominan diagonal** | `|a_ii| ≳ Σ|a_ij|` | pivoting diagonal biasanya aman |
| **Pola tetap** | struktur jarang berubah antar-timestep | **reuse ordering** itu kunci |

---

## 2. Bentuk dasar: LU dan substitusi (dense)

Untuk matriks padat kecil, teori dasarnya:

**Eliminasi Gauss → faktorisasi LU:**

```text
A = P · L · U
```

- `P` — matriks permutasi (dari pivoting).
- `L` — segitiga bawah, diagonal 1.
- `U` — segitiga atas.

Menyelesaikan `A x = b` menjadi dua langkah:

```text
L y = P b     (forward substitution)   O(n²)
U x = y       (back substitution)      O(n²)
```

dengan biaya faktorisasi `O(n³)`.

**Partial pivoting.** Baris ditukar agar pivot `|a_ii|` adalah **terbesar di
kolomnya**. Ini mengendalikan *element growth* (`‖U‖ / ‖A‖`), sumber utama galat
pembulatan. Teori rujukannya: analisis galat Wilkinson (backward error, growth
factor).

Di ngspice jalur ini ada di `maths/dense/dense.c` dan dipakai untuk matriks kecil
(mis. noise/pole-zero dengan dimensi kecil). Untuk `tw-num` ini **N1**: sederhana,
mudah diuji melawan solusi eksak, dan menjadi *testbed* sebelum sparse.

---

## 3. Representasi sparse

### 3.1 Format umum (yang akan dipakai `tw-num`)

| Format | Isi | Kelebihan | Kekurangan |
| --- | --- | --- | --- |
| **COO** (triplets) | daftar `(row, col, value)` | mudah dibangun | tidak efisien untuk operasi |
| **CSR** | `row_ptr`, `col_idx`, `values` | matvec & operasi baris cepat | mahal menyisipkan |
| **CSC** | `col_ptr`, `row_idx`, `values` | operasi kolom & LU cepat | — |

LU biasanya memproses **per kolom**, sehingga **CSC** lebih natural. CSR berguna
untuk `A x` (matvec) dan membentuk `Aᵀ`.

### 3.2 Representasi Sparse 1.4 (ngspice)

ngspice memakai **orthogonal linked list** (`spdefs.h`):

```text
FirstInRow[r] ─► [r,c]=v ──NextInRow──► [r,c']=v' ─► NULL
                   │ NextInCol
FirstInCol[c]      ▼
   └───────────► [r",c]=v" ──NextInCol──► ...
```

Setiap `MatrixElement` menyimpan `{Real, Imag, Row, Col, NextInRow, NextInCol}`,
plus array `Diag[]`, `FirstInRow[]`, `FirstInCol[]`. Kelebihan bentuk ini: **bisa
menyisipkan elemen (termasuk fill-in) di tengah faktorisasi tanpa realokasi
array**. Fill-in dilacak terpisah (`FirstFillinListNode`) agar bisa dibuang
(`spStripFills`).

Untuk `tw-num`, bentuk linked-list ini **tidak wajib ditiru**. Alternatif modern:
CSC + alokasi terukur untuk fill-in. Yang penting: struktur data harus mendukung
**penyisipan fill-in** selama faktorisasi.

---

## 4. Fill-in & ordering — inti sparse LU

### 4.1 Mengapa fill-in muncul

Eliminasi pada matriks sparse menghilangkan nol di posisi yang tadinya nol →
**fill-in**. Contoh:

```text
              sebelum           sesudah eliminasi (fill ⊗)
   [ x x . . ]              [ x x . . ]
   [ x x x . ]     elim  →  [ . x x . ]
   [ . x x x ]     baris1   [ . . x x ]
   [ . . x x ]              [ . . . x ]
                                 ↑ fill baru
```

Jumlah fill-in sangat bergantung pada **urutan eliminasi**. Inilah kenapa
*ordering* menjadi inti.

### 4.2 Kriteria Markowitz

Untuk matriks tak simetris, ukuran biaya memilih pivot `a_ij`:

```text
Markowitz product(i,j) = (row_count(i) − 1) · (col_count(j) − 1)
```

`row_count`/`col_count` = jumlah elemen non-nol pada baris/kolom **setelah
mengeluarkan pivot**. Memilih produk terkecil → kerja & fill-in sedikit.

Di ngspice: `CountMarkowitz`, `MarkowitzProducts`, `SearchForPivot`
(`spfactor.c`).

### 4.3 Threshold pivoting (kestabilan + sparsity)

Markowitz murni bisa memilih pivot yang **secara numerik kecil** → tidak stabil.
Solusinya **threshold**: kandidat pivot `a_ij` hanya boleh diterima jika

```text
|a_ij|  ≥  RelThreshold · max|a_kj|      (maksimum sepanjang kolom j)
|a_ij|  ≥  AbsThreshold
```

- `RelThreshold → 1` ≈ *complete pivoting* (paling stabil, paling banyak fill-in).
- `RelThreshold → 0` ≈ *strict Markowitz* (paling sparse, kurang stabil).
- Default ngspice: `DEFAULT_THRESHOLD = 1.0e-3` (`spconfig.h`).

Ini yang disebut **Markowitz ber-threshold** — kombinasi sparsity + stabilitas.

### 4.4 Varian di ngspice

| Opsi (`spconfig.h`) | Default ngspice | Arti |
| --- | --- | --- |
| `DIAGONAL_PIVOTING` | `YES` | utamakan pivot diagonal (matriks admittance nyaris simetris) |
| `MODIFIED_MARKOWITZ` | `NO` | hentikan pencarian pivot lebih awal + tie-break numerik |
| `MODIFIED_NODAL` | `YES` | preorder khusus matriks MNA |
| `DEFAULT_THRESHOLD` | `1.0e-3` | ambang relatif |
| `MAX_MARKOWITZ_TIES`, `TIES_MULTIPLIER` | 100, 5 | batas tie & early-termination |
| `DEFAULT_PARTITION` | `spAUTO_PARTITION` | direct/indirect addressing per baris |

**Diagonal pivoting** penting: karena matriks MNA hampir dominan diagonal,
memilih pivot dari diagonal (bila lolos threshold) memberi ordering cepat dan
akurat. `SearchForSingleton` mencari baris/kolom ber-degree satu terlebih dulu
(paling murah dan aman).

---

## 5. Simbolik vs numerik — kunci performa

Ini gagasan arsitektural paling penting untuk `tw-num`.

```text
        ┌─────────────────────────────┐
        │  ANALISIS SIMBOLIK          │  sekali per topologi
        │  - ordering (Markowitz/AMD) │
        │  - prediksi fill-in         │
        │  - alokasi struktur         │
        └──────────────┬──────────────┘
                       │ SymbolicLu
                       ▼
        ┌─────────────────────────────┐
        │  FAKTORISASI NUMERIK        │  tiap langkah/timestep
        │  - hitung L, U              │
        │  - pakai ordering yang sama │
        └─────────────────────────────┘
```

Ketika **nilai** `A` berubah tetapi **polanya** tetap (situasi tiap iterasi NR
dan tiap timestep), ordering lama masih valid → cukup faktorisasi numerik.
ngspice mengekspresikan ini lewat:

- `spOrderAndFactor` — memilih ordering dan memfaktor (mahal).
- `spFactor` — **hanya** memfaktor dengan ordering yang sudah ada (lebih cepat).
- `NeedsOrdering` — flag yang memaksa reorder bila struktur berubah atau pivot
  gagal threshold.
- `spFactor` memanggil `spOrderAndFactor` otomatis hanya bila perlu.

Di lapisan engine: `SMPreorder` dan flag `NIACSHOULDREORDER`.

> **Implikasi `tw-num`:** solusi harus punya dua tahap eksplisit —
> `analyze(A) -> SymbolicLu` dan `factorize(A, &SymbolicLu) -> NumericLu`.
> Ini juga mempermudah pengukuran fill-in dan pengujian deterministik.

---

## 6. Stabilitas numerik

- **Growth factor.** `ρ = max|U| / max|A|`; pivoting yang baik menekan `ρ`.
- **Kapan menaikkan threshold.** Jika `A` tidak punya diagonal kuat, naikkan
  `RelThreshold` (mis. `0.01`–`0.1`). Trade-off: lebih stabil vs lebih banyak
  fill-in.
- **Deteksi singular.** ngspice: `MatrixIsSingular`, `ZeroPivot`, `SingularRow`,
  `SingularCol`; error `spSINGULAR` / `spSMALL_PIVOT`.
- **Estimasi kondisi.** `spCondition`, `spPseudoCondition` (opsional) memberi
  indikator ill-conditioning.

Untuk `tw-num`, galat harus **bertipe** (bukan `panic!`):

```rust
pub enum LinearError {
    DimensionMismatch { expected: usize, got: usize },
    Singular { step: usize, row: usize, col: usize },
    SmallPivot { step: usize, pivot: f64, threshold: f64 },
    NotConverged { iterations: usize, residual: f64 },   // untuk iteratif
}
```

---

## 7. Metode iteratif (opsional, bukan jalur utama)

- **CG** (Conjugate Gradient) — untuk sistem **simetris positif-definit**;
  butuh matvec, cocok bila `A` sangat besar & terstruktur.
- **GMRES / BiCGSTAB** — untuk tak simetris.
- **Preconditioner** menentukan keberhasilan.

SPICE memakai **direct LU** karena robust untuk matriks MNA tak simetris dan
karena dapat memakai ulang ordering. `tw-num` menyimpan iteratif sebagai opsi
(trait solver berbeda), bukan target awal.

---

## 8. Kompleks (untuk AC)

AC analysis membutuhkan `A` **kompleks**. Sparse 1.4 mendukung matriks kompleks
lewat field `Imag` pada `MatrixElement` (`ComplexNumber` di `spdefs.h`) dan
rutin `ComplexRowColElimination`, `SolveComplexMatrix`.

Keputusan `tw-num`: jadikan tipe matriks **generik** atas skalar
`{f64, Complex<f64>}` **sejak awal**, agar dukungan AC tidak menyusul mahal.
Struktur & algoritma identik; hanya aritmatikanya berbeda.

---

## 9. Determinisme

Komputasi `f64` bisa berbeda antar-platform karena:
- urutan penjumlahan pada reduksi sparse,
- pemilihan pivot saat **tie** (`MarkowitzProduct` sama),
- kompilasi/instruksi FP.

Kebijakan `tw-num`:
1. **Tie-break pivot eksplisit & deterministik** (mis. indeks baris/kolom
   terkecil) — jangan bergantung pada urutan alokasi memori.
2. **Urutan reduksi tetap** untuk operasi sparse.
3. **Tanpa fast-math**; kriteria berhenti berbasis toleransi numerik, bukan `==`.
4. Pengujian memakai selisih relatif terhadap solusi eksak, bukan bit-exact.

---

## 10. Implikasi desain untuk `tw-num`

```text
tw-num::linear
├── dense::lu         (N1)  LU + partial pivoting
├── sparse::csc       (N2)  struktur + builder (dari COO/triplets)
├── sparse::symbolic  (N2)  ordering + prediksi fill-in  -> SymbolicLu
├── sparse::numeric   (N2)  faktorisasi memakai ordering -> NumericLu
└── solve             (N2)  forward/back substitution
```

Sketsa API (generik atas skalar `T`):

```rust
pub trait Scalar { /* f64, Complex<f64> */ }

pub struct SparseMatrix<T> { /* CSC */ }

pub struct SymbolicLu { /* permutasi, struktur L/U, alokasi */ }
pub struct NumericLu<T> { /* L, U terfaktor */ }

pub trait LinearSolver<T> {
    type Factors;
    fn analyze(&self, a: &SparseMatrix<T>) -> Result<SymbolicLu, LinearError>;
    fn factorize(&self, a: &SparseMatrix<T>, s: &SymbolicLu) -> Result<Self::Factors, LinearError>;
    fn solve(&self, f: &Self::Factors, b: &[T], x: &mut [T]) -> Result<(), LinearError>;
}
```

Aturan tetap: **tidak ada konsep node/ground/satuan di `tw-num`**. Indeks
baris/kolom adalah abstraksi murni.

---

## 11. Strategi pengujian

| Level | Uji | Kriteria |
| --- | --- | --- |
| Dense | sistem 2×2/3×3, matriks Hilbert kecil | cocok solusi eksak; residual `‖Ax−b‖` kecil |
| Sparse struktur | bangun dari triplets, matvec, transpose | hasil sama dengan dense untuk matriks kecil |
| Ordering/fill-in | Poisson 1D (tridiagonal) & grid 2D | jumlah fill-in wajar; urutan deterministik |
| Stabilitas | matriks butuh pivoting (pivot nol di diagonal) | tidak `panic`; `Singular`/`SmallPivot` benar |
| Reuse ordering | ubah nilai, pola tetap | `factorize` ulang tanpa `analyze`; hasil konsisten |
| Konsistensi | sparse vs dense pada matriks acak kecil | solusi identik dalam toleransi |

---

## 12. Referensi

Implementasi acuan (kode ngspice-47):

- `src/maths/sparse/spdefs.h` — struktur data (`MatrixElement`, `MatrixFrame`).
- `src/maths/sparse/spconfig.h` — opsi & konstanta (threshold, pivoting).
- `src/maths/sparse/spfactor.c` — `spOrderAndFactor`, `spFactor`, pencarian pivot,
  eliminasi baris-kolom, `CreateFillin`.
- `src/maths/sparse/spsolve.c` — forward/back substitution (`spSolve`,
  `spSolveTransposed`).
- `src/maths/sparse/spbuild.c` — pembangunan matriks.
- `src/maths/dense/dense.c` — LU dense.
- `src/maths/KLU/` — alternatif SuiteSparse (BTF + AMD + COLAMD).

Literatur:

- Kundert & Sangiovanni-Vincentelli, *Sparse Matrix Techniques* / **Sparse 1.4**.
- Duff, Erisman, Reid, *Direct Methods for Sparse Matrices*.
- Davis, *Direct Methods for Sparse Linear Systems*.
- Golub & Van Loan, *Matrix Computations* (LU, pivoting, growth factor).
- Wilkinson, *The Algebraic Eigenvalue Problem* (analisis galat).
- Ho, Ruehli, Brennan, *The Modified Nodal Approach to Network Analysis* (1975).

Dokumen internal:

- `linear-algebra-foundations.md` — **fondasi pengertian**: penjelasan istilah
  (fill-in, ordering, Markowitz, simbolik/numerik, MNA, dst.) dari dasar, untuk
  dibaca sebelum dokumen ini.
- `overview.md` — peta besar `tw-num`.
- `time-model.md` — relevansi waktu diskret vs kontinu.
