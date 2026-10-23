# Linear Algebra — Fondasi Pengertian

Dokumen ini adalah **jembatan** antara aljabar linear umum yang sudah Anda kenal
dan `linear-algebra.md` yang padat istilah (fill-in, Markowitz, analisis
simbolik vs numerik, MNA, pivoting ber-threshold, dsb.).

Tujuannya bukan mengulang teori matriks dari nol, melainkan **membangun intuisi**
sehingga setiap istilah di `linear-algebra.md` terasa *"oh, itu maksudnya"* —
bukan sekadar rumus yang dihafal.

> Cara pakai: baca dokumen ini sekali dari atas ke bawah untuk membangun fondasi,
> lalu pakai **§16 (peta istilah)** untuk melompat ke bagian `linear-algebra.md`
> yang sedang Anda baca.

---

## Ringkasan (TL;DR)

- **Semua** yang dilakukan `tw-num` berujung pada: selesaikan `A x = b`.
- Cara klasik menyelesaikannya adalah **eliminasi Gauss**, dan bentuk
  efisiennya adalah **faktorisasi LU**. Anda perlu *menghitung LU sekali*, lalu
  *menyelesaikan banyak `b` dengan murah*.
- Pada matriks besar, mayoritas entri adalah **nol** → pakai **representasi
  sparse**. Masalah baru muncul: eliminasi **mengisi** nol dengan nilai baru
  (**fill-in**).
- Karena fill-in bergantung pada **urutan eliminasi**, kita memilih urutan yang
  baik lewat **ordering**. Untuk matriks tak simetris, ukurannya adalah
  **kriteria Markowitz**.
- Kestabilan numerik dan sparsity saling tarik-menarik; komprominya adalah
  **threshold pivoting**.
- Gagasan arsitektural terpenting: pisahkan **analisis simbolik** (mencari
  ordering; sekali per topologi) dari **faktorisasi numerik** (menghitung nilai;
  tiap langkah), lalu **pakai ulang ordering**.
- Matriks nyata dari rangkaian (**MNA**) itu **sparse, tak simetris, simetris
  struktural, dan hampir dominan diagonal**. Sifat-sifat ini yang menjelaskan
  hampir semua keputusan di `linear-algebra.md`.

---

## 0. Peta: yang sudah Anda tahu → yang akan dibahas

| Anda sudah kenal… | Di `tw-num` menjadi… | Bagian di sini |
| --- | --- | --- |
| Menyelesaikan sistem persamaan | `solve(A x = b)` tiap langkah simulasi | §1 |
| Eliminasi Gauss | faktorisasi **LU** | §2–3 |
| Menukar baris | **partial pivoting** + **growth factor** | §4 |
| Matriks jarang (banyak nol) | **CSR / CSC / COO**, nnz | §5–6 |
| (baru) nol berubah jadi bukan-nol | **fill-in** | §7 |
| (baru) memilih urutan | **ordering / Markowitz** | §8 |
| (baru) stabilitas vs kerapatan | **threshold pivoting** | §9 |
| (baru) kerja sekali vs berulang | **simbolik vs numerik** | §10 |
| Sistem dari rangkaian | **MNA** | §11 |
| Galat pembulatan | **growth factor, condition number** | §12 |
| Metode iteratif | **CG / GMRES / preconditioner** | §13 |
| Bilangan kompleks | AC, **skalar generik** | §14 |

---

## 1. Dari masalah nyata ke `A x = b`

Anda tahu bahwa sistem persamaan linear dapat ditulis:

```text
a11 x1 + a12 x2 + ... + a1n xn = b1
a21 x1 + a22 x2 + ... + a2n xn = b2
...
an1 x1 + an2 x2 + ... + ann xn = bn
```

dan disingkat menjadi:

```text
A x = b
```

- `A` = matriks **koefisien** (dimensi `n × n`),
- `x` = vektor **yang dicari** (yang tidak diketahui),
- `b` = vektor **ruas kanan** (yang diketahui).

Yang penting untuk dihayati: **`x` adalah sekumpulan ketidaktahuan**, bukan
sebuah objek tunggal. Kalau Anda menyelesaikan rangkaian listrik, `x` bisa berarti
*"semua tegangan node dan arus cabang"* sekaligus. `n` bisa ribuan sampai
jutaan.

### 1.1 Apa arti "menyelesaikan"?

Secara geometris, **setiap baris** `ai1 x1 + ... = bi` adalah sebuah
*hyperplane* (bidang datar) di ruang berdimensi `n`. Solusi `x` adalah titik
**perpotongan semua hyperplane itu**.

```text
n = 2 → 2 garis berpotongan di 1 titik
n = 3 → 3 bidang berpotongan di 1 titik
n besar → bayangkan, jangan digambar
```

Tiga kemungkinan:

| Kasus | Arti geometris | Di solver |
| --- | --- | --- |
| **solusi tunggal** | hyperplane bertemu di satu titik | sukses (matriks nonsingular) |
| **tak ada solusi** | hyperplane sejajar / tak pernah bertemu | sistem **inkonsisten** |
| **tak hingga banyak** | hyperplane berimpit / redundan | matriks **singular** (ada arah `x ≠ 0` dengan `A x = 0`) |

Untuk simulator rangkaian, kasus "solusi tunggal" adalah norma. Dua kasus lain
biasanya menandakan **topologi yang salah** (mis. dua sumber tegangan ideal
terhubung langsung) dan harus dilaporkan sebagai galat, bukan `panic!`.

### 1.2 Mengapa ini dipanggil terus-menerus

Inilah alasan seluruh dokumen ini ada. Lihat `overview.md` §1: solver linear
dipanggil **setiap iterasi Newton, setiap timestep transient, dan untuk setiap
analisis**. Jadi mempercepat `solve` berarti mempercepat seluruh simulator.

Pola pemanggilannya:

```text
DC operating point   → solve A x = b  (berulang tiap iterasi NR)
Transient            → solve A x = b  (berulang tiap timestep)
AC                   → solve A x = b  (A kompleks)
Sensitivity          → solve Aᵀ x = b
```

---

## 2. Gagasan inti: ubah matriks menjadi bentuk segitiga

Cara paling dasar menyelesaikan `A x = b` adalah **eliminasi Gauss**. Ide
tunggalnya:

> **Gabungkan persamaan satu sama lain sampai sistemnya menjadi segitiga, lalu
> baca solusinya dari bawah ke atas.**

Mengapa segitiga istimewa? Karena sistem segitiga atas:

```text
u11 x1 + u12 x2 + u13 x3 = y1
          u22 x2 + u23 x3 = y2
                    u33 x3 = y3
```

bisa diselesaikan **tanpa inversi matriks**:

```text
x3 = y3 / u33
x2 = (y2 − u23 x3) / u22
x1 = (y1 − u12 x2 − u13 x3) / u11
```

Proses ini disebut **substitusi mundur** (*back substitution*). Kalau matriksnya
segitiga **bawah**, prosesnya **substitusi maju** (*forward substitution*).

### 2.1 Eliminasi baris: contoh konkret

```text
Sistem awal:
(1)   2x1 + 1x2 − 1x3 =   8
(2)  −3x1 − 1x2 + 2x3 = −11
(3)  −2x1 + 1x2 + 2x3 =  −3
```

**Langkah 1 — hilangkan `x1` dari (2) dan (3):**

```text
m21 = a21/a11 = −3/2 = −1.5
(2) ← (2) − m21·(1)      →   0.5 x2 + 0.5 x3 = 1

m31 = a31/a11 = −2/2 = −1
(3) ← (3) − m31·(1)      →   2 x2 + 1 x3 = 5
```

**Langkah 2 — hilangkan `x2` dari (3):**

```text
m32 = a32/a22 = 2/0.5 = 4
(3) ← (3) − m32·(2)      →   −1 x3 = 1
```

**Sekarang berbentuk segitiga atas → substitusi mundur:**

```text
x3 = 1 / (−1)              = −1
x2 = (1 − 0.5·x3) / 0.5    =  3
x1 = (8 − 1·x2 + 1·x3) / 2 =  2
```

Bilangan `m21`, `m31`, `m32` disebut **multiplier**. Merekalah yang disimpan
menjadi matriks `L` ketika kita beralih ke faktorisasi LU (§3).

### 2.2 Biaya

Untuk matriks padat (`n × n`), eliminasi butuh kira-kira:

```text
≈ n³ / 3  perkalian      (faktorisasi)
≈ n²      penjumlahan    (substitusi)
```

Jadi biaya tumbuh **kubik** terhadap ukuran. Ini penting: itulah sebabnya kita
tidak pernah "sekadar memakai invers", dan itu pula kenapa **fill-in** (§7)
sangat mahal kalau tidak dikendalikan.

---

## 3. Faktorisasi LU: memisahkan "cara" dari "data"

Eliminasi Gauss menyisakan jejak: **multiplier** `m` di setiap langkah. Kalau
multiplier itu kita simpan, muncul dekomposisi:

```text
A = L · U
```

- `L` — segitiga bawah, diagonal 1, **isinya multiplier** yang dipakai
  mengeliminasi.
- `U` — segitiga atas, hasil akhir eliminasi.

Ini disebut **faktorisasi LU**. Kuncinya: **proses faktorisasi tidak memakai `b`
sama sekali**. `L` dan `U` hanya bergantung pada `A`.

### 3.1 Mengapa ini penting: banyak `b`, satu `A`

Menyelesaikan `A x = b` menjadi **dua langkah substitusi**:

```text
A x = b
L U x = b
   misalkan U x = y

L y = b     → forward substitution   (murah, O(n²))
U x = y     → back substitution      (murah, O(n²))
```

Jadi:

```text
Faktorisasi A = L U   :  mahal, O(n³), SEKALI
Substitusi tiap solve :  murah, O(n²), BERULANG-ULANG
```

Ini persis pola yang dibutuhkan simulator: **`A` berubah sedikit, tetapi kita
menyelesaikan banyak `b`** (tiap iterasi NR, tiap timestep). Menyimpan LU jauh
lebih murah daripada memfaktor ulang dari nol setiap kali.

### 3.2 Catatan kejujuran soal notasi `A = P·L·U`

`linear-algebra.md` §2 menulis `A = P · L · U`. Perlu diketahui bahwa bentuk
**yang paling tepat secara buku teks** adalah:

```text
P · A = L · U          (P adalah matriks permutasi dari pivoting)
```

atau ekuivalen:

```text
A = Pᵀ · L · U = P⁻¹ · L · U
```

Karena `P` adalah permutasi, `Pᵀ = P⁻¹`. Jadi `A = P·L·U` di dokumen itu
sebaiknya dibaca sebagai **singkatan longgar** ("ada permutasi entah di mana").
Yang esensial untuk dipegang: **pivoting = baris ditukar, dan penukaran itu
dicatat oleh sebuah matriks permutasi**. Jangan sampai notasi ini membuat Anda
bingung ketika bertemu implementasi yang menyimpan `P A = L U`.

---

## 4. Pivoting: kenapa kita menukar baris

Bandingkan dua sistem yang **sama** (hanya urutan barisnya berbeda):

```text
(a)                           (b)
0.0001 x1 + 1 x2 = 1          1 x1 + 1 x2 = 2
      1 x1 + 1 x2 = 2          0.0001 x1 + 1 x2 = 1
```

Solusi eksaknya sama: `x1 ≈ 1.0001`, `x2 ≈ 0.9999`. Tetapi tanpa penukaran
baris, pivot `a11 = 0.0001` memaksa **multiplier** `1 / 0.0001 = 10⁴`:

```text
(a)  row2 ← row2 − 10000 · row1
     (1 − 10000·1) x2 = 2 − 10000·1
             −9999 x2 = −9998          →  x2 ≈ 0.9999
```

Masalahnya bukan hasil aritmetika eksak (yang benar), tetapi **pembulatan**.
Kedua ruas kini didominasi angka ~`10⁴` yang hampir sama besar; informasi tentang
`0.0001` tersimpan hanya di digit-digit terakhir. Setiap galat pembulatan kecil
pada data ikut **diperbesar ~10⁴**. Lalu `x1 = (1 − x2) / 0.0001` memperbesar
galat itu **sekali lagi** (di sini `1 − x2` adalah selisih dua angka ~1 yang
hampir sama — *catastrophic cancellation*).

Dengan menukar baris seperti (b), pivot menjadi `1` dan multiplier menjadi
`0.0001` yang kecil → **tidak ada pembesaran**. Inilah seluruh alasan pivoting
ada.

### 4.1 Partial vs complete pivoting

| Strategi | Cara memilih pivot | Catatan |
| --- | --- | --- |
| **Tanpa pivoting** | selalu `a_ii` | bisa gagal / tak stabil |
| **Partial pivoting** | cari `max` di **kolom** yang sama, tukar **baris** | standar; hampir selalu cukup |
| **Complete pivoting** | cari `max` di seluruh submatriks, tukar **baris & kolom** | paling stabil, paling mahal & mengacaukan struktur sparse |

`tw-num` N1 memakai **partial pivoting**: rasio manfaat/mudah yang terbaik untuk
matriks padat kecil.

### 4.2 Growth factor

Efek hilang digit di atas diukur oleh **growth factor**:

```text
ρ = (elemen terbesar di U) / (elemen terbesar di A)
```

Pivoting yang baik menahan `ρ` tetap kecil (partial pivoting menjamin
`ρ ≤ 2^{n−1}` dalam teori terburuk, tetapi praktisnya jauh lebih kecil pada
matriks asli). Ini "sumber utama galat pembulatan" yang disebut di
`linear-algebra.md` §2 dan §6.

---

## 5. Dari dense ke sparse: nol adalah aset

Matriks **padat** (dense) menyimpan semua `n²` entri. Untuk `n = 10000`, itu
**100 juta** `f64` ≈ 800 MB — boros, dan lebih parah lagi: sebagian besar
entri itu **nol**.

Matriks dari rangkaian listrik hampir selalu **sparse**: setiap node hanya
terhubung ke segelintir komponen. Kalau jumlah non-nol per baris rata-rata ≈ 4,
maka dari 100 juta entri, hanya ≈ 400 ribu yang bukan nol:

```text
dense :  n²       ≈ 1.0e8  entri
sparse:  nnz      ≈ 4.0e5  entri      (≈ 250× lebih hemat)
```

**Non-nol (nnz)** adalah metrik sentral. File `linear-algebra.md` §1 menyebut
matriks "sparse"; itu artinya `nnz ≪ n²`.

Konsekuensinya:

1. Simpan **hanya** non-nol (jangan array `n × n`).
2. Algoritma harus **sadar struktur**: jangan iterasi semua entri, iterasi hanya
   yang ada.
3. Biaya dan memori ikut `nnz`, bukan `n²`.

---

## 6. Format penyimpanan sparse

Bagaimana menyimpan hanya non-nolnya? Tiga format yang disebut di
`linear-algebra.md` §3.1:

### 6.1 COO (coordinate / triplet)

Simpan daftar `(row, col, value)`. Contoh matriks

```text
      col 0  1  2
row 0 [  5  .  . ]
row 1 [  .  8  1 ]
row 2 [  .  .  3 ]
```

menjadi:

```text
rows    = [0, 1, 1, 2]
cols    = [0, 1, 2, 2]
values  = [5, 8, 1, 3]
```

Kelebihan: **sangat mudah dibangun** (cukup `push` triplet). Kekurangan: sulit
diakses/dioperasikan. Karena itu COO dipakai sebagai **format input/pembangun**,
bukan format kerja.

### 6.2 CSR (Compressed Sparse Row)

Tiga array: `row_ptr`, `col_idx`, `values`. Untuk matriks di atas:

```text
row_ptr  = [0, 1, 3, 4]     # baris i menempati irisan [row_ptr[i], row_ptr[i+1])
col_idx  = [0, 1, 2, 2]
values   = [5, 8, 1, 3]
```

Artinya: baris 0 → entri ke-0 (`col 0 = 5`); baris 1 → entri ke-1..2
(`col 1 = 8`, `col 2 = 1`); baris 2 → entri ke-3 (`col 2 = 3`).
Perhatikan **kolom urut menaik** — itu mempermudah pencarian.

Cocok untuk **operasi per-baris**: `A x` (matvec), reduksi baris, membentuk `Aᵀ`.

### 6.3 CSC (Compressed Sparse Column)

Sama seperti CSR tetapi **kolom** yang dipadatkan: `col_ptr`, `row_idx`,
`values`.

```text
col_ptr  = [0, 1, 2, 4]
row_idx  = [0, 1, 1, 2]
values   = [5, 8, 1, 3]
```

Cocok untuk **operasi per-kolom**. **LU biasanya memproses per kolom**, jadi
**CSC lebih natural untuk faktorisasi**. Inilah alasan `tw-num` memilih CSC
sebagai struktur kerja (§10 di `linear-algebra.md`).

### 6.4 Ringkas

| Format | Isi | Paling cocok untuk | Paling lemah untuk |
| --- | --- | --- | --- |
| **COO** | daftar triplet | membangun dari nol | hampir semua operasi |
| **CSR** | per-baris | `A x`, bentuk `Aᵀ`, operasi baris | menyisipkan elemen |
| **CSC** | per-kolom | LU, substitusi kolom | menyisipkan elemen |

Catatan penting: masuknya **fill-in** (§7) berarti struktur berubah saat
faktorisasi — sesuatu yang "menyisipkan elemen" itu justru mahal di CSR/CSC.
Karena itu `tw-num` akan memakai CSC + **alokasi terukur** untuk fill-in yang
sudah diprediksi lebih dulu (§10).

---

## 7. Fill-in: fenomena yang mengubah segalanya

Ini konsep **paling penting** untuk memahami mengapa ada seluruh mesin ordering
di `linear-algebra.md` §4.

### 7.1 Apa itu fill-in

Saat mengeliminasi variabel, operasi `baris_i ← baris_i − m · baris_pivot` bisa
menghasilkan nilai di posisi yang **tadinya nol**. Nol itu "terisi" — itulah
**fill-in**.

Contohkan secara grafik. Pandang matriks yang polanya seperti **bintang**
(node 1 terhubung ke 2, 3, 4):

```text
        Pola awal A                    Graf-nya
          1  2  3  4
    1  [  x  x  x  x ]                    1
    2  [  x  x  .  . ]                  / | \
    3  [  x  .  x  . ]                 2  3  4
    4  [  x  .  .  x ]
```

Sekarang **eliminasi node 1 dulu**. Node 1 adalah satu-satunya perantara antara
2, 3, 4; setelah ia hilang, 2, 3, 4 menjadi saling terhubung:

```text
        Setelah eliminasi 1           Graf: 2,3,4 jadi clique
          1  2  3  4
    1  [  x  x  x  x ]                    1 (keluar)
    2  [  .  x  x  x ]                   2
    3  [  .  x  x  x ]                  /|\
    4  [  .  x  x  x ]                 3—•—4
                                     (3, 4, dan pasangan) terisi
```

Entri `(2,3)`, `(2,4)`, `(3,4)` yang tadinya nol **kini berisi** (fill-in).
Matriks jadi jauh lebih padat.

### 7.2 Aturan grafis

```text
"Mengeliminasi sebuah simpul = menghubungkan semua tetangganya menjadi clique."
Setiap sisi baru yang muncul = fill-in.
```

### 7.3 Kenapa urutan itu menentukan segalanya

Sekarang eliminasi **daun** (node 2) lebih dulu:

```text
Node 2 hanya bertetangga dengan 1 → tidak ada pasangan baru → TIDAK ada fill-in.
```

Hasilnya:

| Urutan eliminasi | Fill-in yang muncul |
| --- | --- |
| 2, 3, 4 dulu (daun-daun) | **0** |
| 1 dulu (pusat) | 3 (dan seterusnya, makin buruk untuk bintang besar) |

Untuk bintang dengan `k` daun, mengeliminasi pusat lebih dulu menghasilkan
≈ `k²/2` fill-in; mengeliminasi daun lebih dulu menghasilkan **nol**. Perbedaan
raksasa hanya dari **urutan**.

> Inilah kalimat "Jumlah fill-in sangat bergantung pada urutan eliminasi" di
> `linear-algebra.md` §4.1. **Ordering = seni memilih urutan eliminasi yang
> meminimalkan fill-in.**

---

## 8. Ordering: memilih urutan yang baik

### 8.1 Ordering = permutasi = pelabelan ulang

Memilih urutan eliminasi setara dengan **memberi nomor baru** pada baris dan
kolom. Secara matematis ini **permutasi simetris**:

```text
A' = P · A · Pᵀ
```

Untuk matriks simetris struktural (pola nolnya simetris), ini sekadar
**melabel ulang simpul graf**; grafnya tidak berubah, hanya nomornya. Tujuan:
menemukan pelabelan yang meminimalkan fill-in.

### 8.2 Minimum degree (intuisi)

Aturan *greedy* paling intuitif:

> **Selalu eliminasi simpul dengan derajat (jumlah tetangga) paling kecil.**

Alasan: sederhana — biaya fill-in lokal itu kira-kira kuadratik terhadap derajat
simpul. Dari §7, menghubungkan `d` tetangga jadi clique memunculkan ≈ `d²/2`
sisi baru. Jadi kecilkan `d`.

Ini disebut **Minimum Degree**, dan varian modernnya **AMD** (Approximate
Minimum Degree) sering dipakai dalam paket seperti KLU/SuiteSparse.

### 8.3 Markowitz: versi tak simetris

Untuk matriks **tak simetris** (kasus umum di MNA, karena ada sumber tegangan),
"derajat" saja tidak cukup: baris dan kolom bisa punya jumlah non-nol berbeda.
Ukuran biaya memilih pivot di `(i, j)` adalah **Markowitz product**:

```text
Markowitz(i, j) = (row_count(i) − 1) · (col_count(j) − 1)
```

dengan `row_count(i)` = banyak non-nol di baris `i` **setelah pivot dikeluarkan**,
dan `col_count(j)` serupa. Produk kecil ⇒ fill-in dan kerja kecil.

```text
Perhatikan: kalau matriksnya simetris (row_count = col_count = d),
Markowitz menjadi (d−1)² — sejalan dengan "kecilkan derajat" versi Minimum Degree.
```

Di ngspice ini muncul sebagai `CountMarkowitz`, `MarkowitzProducts`,
`SearchForPivot` (`spfactor.c`).

### 8.4 Singletons & diagonal pivoting

Dua percepatan penting di `linear-algebra.md` §4.4:

- **Singleton** (`SearchForSingleton`): baris/kolom dengan tepat **satu** non-nol.
  Mengeliminasinya **tidak menghasilkan fill-in sama sekali** dan pasti stabil
  (tidak ada pilihan lain) → kerjakan lebih dulu. Contohnya: baris dengan satu
  `1` dari sumber tegangan.
- **Diagonal pivoting** (`DIAGONAL_PIVOTING = YES`): utamakan kandidat pivot **di
  diagonal** `a_ii`. Alasannya berasal dari §11: matriks MNA hampir dominan
  diagonal, jadi diagonal biasanya besar & aman. Ini mempercepat pencarian
  pivot (tidak perlu menyisir seluruh kolom).

### 8.5 Varian lain

`MODIFIED_MARKOWITZ` menghentikan pencarian pivot lebih awal begitu menemukan
kandidat yang "cukup baik" + tie-break numerik; `MODIFIED_NODAL` melakukan
preorder khusus bentuk MNA. Semuanya adalah **heuristik untuk satu tujuan yang
sama**: fill-in sekecil mungkin dengan usaha mencari yang wajar.

---

## 9. Threshold pivoting: tarik-menarik sparsity vs kestabilan

Ada konflik mendasar:

```text
Markowitz murni  → paling sparse,  TAPI bisa memilih pivot yang sangat kecil
                                   → tidak stabil (§4).
Partial pivoting → paling stabil,  TAPI mengabaikan sparsity → banyak fill-in.
```

Jalan tengah: **threshold pivoting**. Kandidat pivot `a_ij` diterima hanya jika
**cukup besar** dibanding kolomnya:

```text
|a_ij| ≥ RelThreshold · max|a_kj|      (maksimum sepanjang kolom j)
|a_ij| ≥ AbsThreshold                  (ambang absolut)
```

Artinya: kita baru **boleh** memilih pivot yang sparse, asalkan nilai numeriknya
tidak terlalu jauh di bawah elemen terbesar kolomnya.

### 9.1 Membaca nilai `RelThreshold`

| Nilai | Perilaku | Efek |
| --- | --- | --- |
| `→ 1` | pivot harus (hampir) terbesar sekolom → ≈ **complete pivoting** | stabil, banyak fill-in |
| `→ 0` | hampir bebas → ≈ **strict Markowitz** | sparse, kurang stabil |
| `1e-3` | **default ngspice** (`DEFAULT_THRESHOLD`) | kompromi seimbang |

Contoh konkret: kolom `j` punya `max|a_kj| = 1`, dan `RelThreshold = 1e-3`.
Kandidat `a_ij = 0.001` **lolos** (karena `0.001 ≥ 0.001`), tetapi `a_ij = 5e-4`
**ditolak**. Jadi threshold bertindak seperti "saringan kelayakan" sebelum
Markowitz memilih di antara kandidat yang lolos.

> **Kapan menaikkan threshold?** Bila `A` tidak punya diagonal yang kuat
> (`linear-algebra.md` §6). Lebih stabil, tetapi lebih banyak fill-in.

---

## 10. Analisis simbolik vs numerik: kunci performa

Ini gagasan arsitektural **paling penting** untuk `tw-num`, dan alasan `tw-num`
harus punya API dua tahap.

### 10.1 Pengamatan kunci

Dalam simulator, **pola** matriks (posisi mana yang non-nol) hampir selalu
**tetap**, hanya **nilainya** yang berubah:

```text
iterasi NR ke-1:  A punya pola P, nilai v1
iterasi NR ke-2:  A punya pola P, nilai v2   ← pola sama!
timestep berikut: A punya pola P, nilai v3   ← pola masih sama!
```

Padahal **ordering** (dan prediksi fill-in) hanya bergantung pada **pola**,
bukan nilai. Jadi kita bisa menghitungnya sekali dan **memakainya ulang**.

### 10.2 Dua fase

```text
        ┌─────────────────────────────┐
        │  ANALISIS SIMBOLIK          │  sekali per topologi (mahal, tapi jarang)
        │  - ordering (Markowitz/AMD) │
        │  - prediksi fill-in         │
        │  - alokasi struktur         │
        └──────────────┬──────────────┘
                       │  SymbolicLu
                       ▼
        ┌─────────────────────────────┐
        │  FAKTORISASI NUMERIK        │  tiap langkah/timestep (murah, sering)
        │  - hitung nilai L, U        │
        │  - pakai ordering yang sama │
        └─────────────────────────────┘
```

| Fase | Masukan | Menghasilkan | Frekuensi |
| --- | --- | --- | --- |
| **Simbolik** (`analyze`) | pola `A` | `SymbolicLu` (permutasi, struktur L/U, alokasi) | sekali per topologi |
| **Numerik** (`factorize`) | `A` + `SymbolicLu` | `NumericLu` (nilai L, U) | tiap langkah |
| **Solve** | `NumericLu` + `b` | `x` | tiap solve |

Di ngspice ini diekspresikan oleh `spOrderAndFactor` (mahal: ordering + faktor),
`spFactor` (**hanya** faktor dengan ordering lama; cepat), dan flag
`NeedsOrdering` yang memaksa reorder hanya bila struktur berubah atau pivot
gagal threshold.

### 10.3 Kapan ordering HARUS dihitung ulang

- **topologi berubah** (rangkaian berbeda / switch berubah konfigurasi),
- **pivot gagal threshold** (kestabilan menuntut urutan lain),
- pola non-nol berubah.

Selain itu, ordering lama tetap valid. Inilah yang membuat "reuse ordering" jadi
kunci performa SPICE (`linear-algebra.md` §1, §5).

### 10.4 Implikasi API

```rust
// Sketsa dari linear-algebra.md §10
let sym   = solver.analyze(&a)?;              // simbolik: sekali
let lu    = solver.factorize(&a, &sym)?;      // numerik: tiap perubahan nilai
solver.solve(&lu, &b, &mut x)?;               // solve: murah
```

Pemisahan ini juga mempermudah **pengukuran fill-in** dan **pengujian
deterministik** (§15).

---

## 11. Dari mana matriks ini datang: MNA

Supaya sifat-sifat matriks di §1 `linear-algebra.md` tidak terasa seperti
"datang dari langit", ini asal-usulnya: **MNA** (Modified Nodal Analysis,
Ho–Ruehli–Brennan 1975).

### 11.1 Gagasan MNA

- **Yang tidak diketahui (`x`)**: semua **tegangan node** + **arus** pada sumber
  tegangan (dan elemen sejenis). Arus ekstra ini yang membuat MNA "modified".
- **Persamaan**: **KCL** (hukum arus Kirchhoff) di setiap node, ditambah
  persamaan yang memaksa beda tegangan pada sumber tegangan.
- Hasilnya berbentuk blok:

```text
[ G  B ] [ v ]   [ i ]
[ C  D ] [ j ] = [ e ]
```

dengan `G` bagian konduktansi (simetris), `B`/`C` keterkaitan sumber tegangan
(sering kali `C = Bᵀ`), `D` biasanya nol.

### 11.2 Mengapa sifat-sifat §1 muncul

| Sifat | Asal-usul di MNA | Konsekuensi |
| --- | --- | --- |
| **Sparse** | tiap node hanya terhubung sedikit komponen | butuh format sparse (§5–6) |
| **Tak simetris** | sumber tegangan & sumber terkendali merusak simetri nilai | butuh **LU**, bukan Cholesky |
| **Simetris struktural** | `C = Bᵀ` ⇒ posisi nol simetris, walau nilainya tidak | `P A Pᵀ` = relabel, ordering murah (§8.1) |
| **Hampir dominan diagonal** | `|a_ii|` = jumlah konduktansi di node itu | pivot diagonal biasanya aman (§8.4) |
| **Pola tetap antar-timestep** | topologi tak berubah | **reuse ordering** (§10) |

**Dominan diagonal** berarti `|a_ii| ≥ Σ_{j≠i} |a_ij|` (setidaknya kira-kira).
Ini penting: ia menjamin matriks nonsingular dan membuat `a_ii` kandidat pivot
yang bagus.

### 11.3 Ada juga KLU

`linear-algebra.md` §12 menyebut `src/maths/KLU/`: itu alternatif gaya
SuiteSparse yang memakai **BTF + AMD + COLAMD** (ordering berbasis Minimum
Degree) alih-alih Markowitz. Target awalnya tetap **Markowitz** (kanonik
SPICE), AMD dipertimbangkan kemudian.

---

## 12. Stabilitas numerik & galat

`f64` tidak menyimpan bilangan real dengan tepat; setiap operasi punya galat
pembulatan ≈ `2.2e-16` (machine epsilon). Pemahaman yang perlu dibangun:

- **Backward error**: "Solusi yang kita dapat adalah solusi eksak dari masalah
  yang sedikit terganggu." LU dengan partial pivoting punya backward error yang
  baik; itu sebabnya ia andal.
- **Forward error** (galat solusi) dikendalikan oleh **condition number**:

```text
κ(A) = ‖A‖ · ‖A⁻¹‖

galat relatif solusi  ≲  κ(A) · galat relatif data
```

`κ(A)` besar ⇒ **ill-conditioned**: perubahan kecil pada `b`/`A` bisa mengubah
`x` besar-besaran. Matriks MNA dengan konduktansi berskala sangat berbeda
(mis. `1e-9` sampai `1e3`) cenderung ber-kondisi buruk — inilah alasan nyata di
balik kekhawatiran akan kestabilan dan pemilihan threshold (§9).

- **Growth factor** (§4.2) mengukur pembesaran galat selama eliminasi.

### 12.1 Singular & pivot kecil

Kalau pivot jadi nol (atau sangat kecil), matriksnya **singular / nyaris
singular**. ngspice mendeteksi lewat `MatrixIsSingular`, `ZeroPivot`,
`SingularRow`, `SingularCol`, dengan galat `spSINGULAR` / `spSMALL_PIVOT`.

Kebijakan `tw-num`: galat **bertipe**, bukan `panic!`:

```rust
pub enum LinearError {
    DimensionMismatch { expected: usize, got: usize },
    Singular   { step: usize, row: usize, col: usize },
    SmallPivot { step: usize, pivot: f64, threshold: f64 },
    NotConverged { iterations: usize, residual: f64 },   // untuk iteratif
}
```

Alasannya: simulator harus bisa **melaporkan** "rangkaian singular" sebagai
diagnostik yang ramah, bukan crash.

---

## 13. Direct vs iteratif

### 13.1 Metode langsung (direct)

Faktorisasi + substitusi (§3). Untuk `A` sparse dari rangkaian:

- **Robust** untuk matriks tak simetris,
- **dapat memakai ulang ordering** (§10),
- biaya bergantung pada fill-in yang sudah dikendalikan.

Ini **jalur utama** `tw-num`.

### 13.2 Metode iteratif

Mendekati `x` secara bertahap lewat perkalian matriks-vektor (`A x`), tanpa
faktorisasi:

| Metode | Untuk | Syarat |
| --- | --- | --- |
| **CG** (Conjugate Gradient) | sistem **simetris positif-definit** | butuh matvec; konvergen cepat bila ber-kondisi baik |
| **GMRES** | tak simetris | menyimpan basis Krylov |
| **BiCGSTAB** | tak simetris, memori lebih hemat | bisa tidak stabil |

Keberhasilannya **bergantung preconditioner** (pendekatan `M⁻¹` yang mempermudah
sistem tanpa mengubah solusinya). Menentukan preconditioner yang baik itu seni.

Mengapa bukan jalur utama SPICE? Karena matriks MNA **tak simetris** (CG tidak
langsung berlaku) dan karena direct LU bisa **memakai ulang ordering** —
keunggulan besar yang tidak dimiliki iteratif. `tw-num` menyimpannya sebagai
opsi dengan trait solver terpisah (`linear-algebra.md` §7).

---

## 14. Bilangan kompleks (untuk AC)

Analisis AC butuh `A` **kompleks**, karena impedansi punya suku imajiner
(`jωL`, `1/(jωC)`). Kabar baiknya: **seluruh teori di atas tetap berlaku**.

Dua cara memandangnya:

1. **Sebagai bilangan kompleks** `a + bj`. Sparse 1.4 melakukan ini: setiap
   `MatrixElement` punya `{Real, Imag}` (`ComplexNumber` di `spdefs.h`), dengan
   rutin `ComplexRowColElimination`, `SolveComplexMatrix`.
2. **Sebagai matriks real 2×2** yang menyandi `(a+bi)`:

```text
a + bi   ↔   [ a  −b ]
             [ b   a ]
```

Karena struktur & algoritmanya identik — hanya aritmetikanya berbeda —
`tw-num` menjadikan tipe skalar **generik** atas `{f64, Complex<f64>}` **sejak
awal**. Dengan begitu dukungan AC tidak menjadi pekerjaan besar yang menyusul.

---

## 15. Determinisme

Komputasi `f64` bisa memberi hasil berbeda antar-platform, karena penjumlahan
bilangan pecahan bergantung urutan (*floating point tidak asosiatif*:
`(a+b)+c ≠ a+(b+c)`). Pemicunya:

- urutan penjumlahan saat reduksi sparse,
- pemilihan pivot saat **tie** (Markowitz product sama),
- perbedaan kompilasi/instruksi FP.

Kebijakan `tw-num` (dari `linear-algebra.md` §9):

1. **Tie-break pivot eksplisit**: mis. pilih indeks baris/kolom terkecil —
   jangan bergantung pada urutan alokasi memori.
2. **Urutan reduksi tetap** untuk operasi sparse.
3. **Tanpa fast-math**; kriteria berhenti berbasis **toleransi numerik**, bukan
   `==`.
4. Pengujian membandingkan **selisih relatif** terhadap solusi eksak, bukan
   bit-exact.

---

## 16. Peta istilah → bagian rujukan

Pakai tabel ini saat membaca `linear-algebra.md`: cari istilah yang membingungkan,
lalu buka bagian fondasinya.

| Istilah di `linear-algebra.md` | Artinya | Di sini |
| --- | --- | --- |
| `A x = b` | sistem linear; `x` = yang dicari | §1 |
| **LU** | faktorisasi `PA = LU` | §2–3 |
| **partial pivoting** | tukar baris agar pivot terbesar | §4 |
| **growth factor** `ρ` | pembesaran elemen selama eliminasi | §4.2 |
| **COO / CSR / CSC** | format simpan sparse | §6 |
| **nnz** | jumlah entri bukan-nol | §5 |
| **fill-in** | nol yang terisi saat eliminasi | §7 |
| **ordering** | pemilihan urutan eliminasi (permutasi) | §8 |
| **Markowitz product** | `(row−1)(col−1)` | §8.3 |
| **singleton** | baris/kolom dengan 1 non-nol | §8.4 |
| **minimum degree / AMD** | heuristik ordering simetris | §8.2 |
| **threshold pivoting** | saringan kestabilan sebelum pilih pivot | §9 |
| **analisis simbolik** | hitung ordering + prediksi fill-in | §10 |
| **faktorisasi numerik** | hitung nilai L, U | §10 |
| **reuse ordering** | pakai ordering lama antar-timestep | §10 |
| **MNA** | asal-usul matriks rangkaian | §11 |
| **dominasi diagonal** | `|a_ii| ≥ Σ|a_ij|` | §11.2 |
| **condition number** `κ(A)` | kepekaan solusi terhadap galat | §12 |
| **singular** | tak ada solusi tunggal | §1.1, §12.1 |
| **CG / GMRES / preconditioner** | metode iteratif | §13 |
| **kompleks (AC)** | skalar `Complex<f64>` | §14 |
| **tie-break deterministik** | aturan seri agar hasil konsisten | §15 |

---

## 17. Cara membaca `linear-algebra.md` dengan dokumen ini

| Bagian `linear-algebra.md` | Baca fondasi dulu |
| --- | --- |
| §1 Peran & konteks, sifat matriks | §1 + §11 |
| §2 LU & substitusi dense | §2–§4 |
| §3 Representasi sparse | §5–§6 |
| §4 Fill-in & ordering (Markowitz, threshold) | §7–§9 |
| §5 Simbolik vs numerik | §10 |
| §6 Stabilitas numerik | §4.2 + §12 |
| §7 Metode iteratif | §13 |
| §8 Kompleks (AC) | §14 |
| §9 Determinisme | §15 |
| §10 Implikasi desain | §10.4 |

---

## 18. Referensi

Literatur yang menjelaskan topik di atas secara lebih formal:

- Golub & Van Loan, *Matrix Computations* — LU, pivoting, growth factor,
  condition number.
- Duff, Erisman, Reid, *Direct Methods for Sparse Matrices* — fill-in, ordering,
  Markowitz, analisis simbolik vs numerik.
- Davis, *Direct Methods for Sparse Linear Systems* — algoritma sparse modern.
- Wilkinson, *The Algebraic Eigenvalue Problem* — analisis galat mundur.
- Kundert & Sangiovanni-Vincentelli, *Sparse Matrix Techniques* / Sparse 1.4.
- Ho, Ruehli, Brennan, *The Modified Nodal Approach to Network Analysis* (1975).
- Najm, *Circuit Simulation* — konteks MNA dan simulasi rangkaian.

Dokumen internal terkait:

- `overview.md` — peta besar `tw-num`.
- `linear-algebra.md` — dokumen target: teori & implikasi desain.
- `determinism.md` — kebijakan reproduktibilitas `f64` (rencana).
