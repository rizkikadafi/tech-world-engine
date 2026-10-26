# Konsep Dasar: Resistor, MNA, dan RC Transient (Backward Euler)

Dokumen ini menjelaskan konsep yang dipakai di test `tw-num/tests/resistor_divider.rs` dan `tw-num/tests/rc_transient_be.rs`. Tujuannya biar lebih gampang dipahami kenapa kita butuh solver linear (`A x = b`) buat "menghitung" rangkaian listrik, bukan cuma bilang HIGH/LOW.

> Catatan: `tw-num` itu sendiri **domain-agnostic**. Semua perhitungan MNA/RC di test ini cuma buat validasi bahwa solver kita beneran bisa dipake nanti di `models/electrical/`.

## 1. Resistor (Ohm's Law)

**Resistor** adalah komponen yang membatasi aliran arus listrik. Hubungannya sangat simpel:

$$
V = I * R
$$

Atau bisa ditulis ulang:

$$
I = \frac{V}{R} = G * V,\quad G = \frac{1}{R}
$$

- $V$ = beda tegangan (volt, V)
- $I$ = arus (ampere, A)
- $R$ = resistansi (ohm, $\Omega$)
- $G$ = konduktansi (siemens, S)

**Intuisi:** Semakin besar R, semakin susah arus mengalir buat beda tegangan yang sama.

## 2. Voltage Divider (Resistor Divider)

Ini kasus paling simpel buat nunjukin kenapa kita pakai persamaan linier.

Rangkaian:

```text
VDD --R1-- node1 --R2-- GND
```

Arus yang sama mengalir lewat R1 dan R2 (seri):

$$
I = \frac{VDD - V_{node1}}{R1} = \frac{V_{node1} - 0}{R2}
$$

Selesaikan:

$$
V_{node1} = VDD * \frac{R2}{R1 + R2}
$$

### Kenapa kita pakai MNA di test?

Di test `resistor_divider.rs` kita nggak langsung pakai rumus di atas. Kita bangun persamaan berbentuk:

$$
G * V = I
$$

Buat kasus 1-node:

- Konduktansi total yang "keluar" ke node: $G = 1/R1 + 1/R2$
- Sumber arus ekuivalen: $I = VDD/R1$ (karena R1 nyambung ke VDD)
- Jadi: $(1/R1 + 1/R2) * V_{node1} = VDD/R1$

Kalo kita pecah, ini ekuivalen sama rumus voltage divider. Bedanya: **kalo rangkaiannya makin kompleks (banyak node, sumber tegangan, dll), kita nggak bisa lagi ngitung manual per rumus. Kita butuh metode umum kayak MNA.**

**MNA (*Modified Nodal Analysis*)** intinya: ubah seluruh rangkaian jadi sistem persamaan linier $A v = b$ dimana $v$ adalah tegangan tiap node (kecuali ground yang kita anggap 0).

## 3. Kapasitor dan RC

**Kapasitor** nyimpen muatan listrik. Berbeda sama resistor yang langsung "ikut" hukum Ohm, kapasitor punya hubungan **terhadap turunan waktu**.

$$
I_C = C \frac{dV_C}{dt}
$$

- $C$ = kapasitansi (farad, F)
- $\frac{dV_C}{dt}$ = laju perubahan tegangan terhadap waktu (V/s)

**Intuisi:** Kapasitor "menahan" perubahan tegangan. Kalo kamu kasih tegangan step, arus awal bisa besar buat "mengisi" muatan, lalu arus makin kecil seiring tegangan mendekati sumber.

### Rangkaian RC Charging

```text
Vsrc (step ke VDD di t=0) -- R -- node(V) -- C -- GND
```

Persamaan KCL di node $V$:

$$
\frac{V - V_{src}}{R} + C\frac{dV}{dt} = 0
$$

Ubah bentuk:

$$
C\frac{dV}{dt} + \frac{V - V_{src}}{R} = 0
$$

Ini adalah **persamaan diferensial biasa (ODE)** orde pertama. Solusi analitiknya buat $V_{src}=VDD, V(0)=0$:

$$
V(t) = V_{DD}\left(1 - e^{-t/(R C)}\right)
$$

$RC$ disebut **time constant** ($\tau$). Setelah $t=\tau$, tegangan udah sekitar 63% dari $VDD$. Setelah $5\tau$, udah sangat dekat (99%+).

## 4. Backward Euler (BE)

Kita nggak bisa "ngitung terus" secara kontinu di komputer. Kita **diskritisasi** waktu jadi langkah-langkah kecil $\Delta t = dt$.

### Forward Euler (buat gambaran)
Coba aproksimasi $\frac{dV}{dt}$ dari masa lalu:

$$
\frac{dV}{dt} \approx \frac{V_{n} - V_{n- ?}}{...}
$$

Lebih umum: $\frac{dV}{dt} \approx \frac{V_{n+1} - V_n}{\Delta t}$? **Forward Euler** biasanya pakai nilai sekarang buat turunan: $\frac{dV}{dt}\Big|_n \approx \frac{V_{n+1}-V_n}{\Delta t}$, tapi ada varian. Yang penting: **Backward Euler** beda.

### Backward Euler
**Backward Euler** mengaproksimasi turunan pakai **nilai di masa depan** ($n+1$):

$$
\frac{dV}{dt} \approx \frac{V_{n+1} - V_n}{\Delta t}
$$

Kenapa "backward"? Karena kita "liat ke depan" buat nyari $V_{n+1}$. Konsekuensinya: kita **harus nyelesain persamaan linier** tiap step.

Masukin ke persamaan RC:

$$
C\frac{V_{n+1} - V_n}{\Delta t} + \frac{V_{n+1} - V_{src}}{R} = 0
$$

Kumpulin yang ada $V_{n+1}$ di kiri, sisanya di kanan:

$$
V_{n+1}\left(\frac{C}{\Delta t} + \frac{1}{R}\right) = \frac{C}{\Delta t}V_n + \frac{V_{src}}{R}
$$

Bentuk ini persis $A V_{n+1} = b$ dengan:

```text
A = [ C/dt + 1/R ]
b = [ (C/dt)*V_n + Vsrc/R ]
```

Jadi tiap iterasi:
1. Hitung $b$ berdasarkan $V_n$ (nilai dari step sebelumnya)
2. Panggil `tw-num::solve(&A, &b)` buat dapetin $V_{n+1}$
3. Set $V_n = V_{n+1}$ buat step berikutnya

## 5. Bedain Kedua Test

| Test | Tipe | Yang Diuji | Penjelasan |
|---|---|---|---|
| **`resistor_divider.rs`** | **Steady-state / DC** | $A v = b$ murni | Hanya "keseimbangan". Nggak ada turunan waktu, arus/tegangan langsung nemuin nilai konstan. Cukup sekali solve. |
| **`rc_transient_be.rs`** | **Transient** | $A x = b$ berulang + time-stepping | Kita lacak perubahan terhadap waktu. Tiap $\Delta t$ kita bentuk persamaan baru ($A$ bisa tetap kalo elemen linier, $b$ berubah) dan solve lagi. |

## 6. Kenapa Ini Relevan Buat Tech World Engine

- **Jembatan ke circuit solver**: Nanti di Phase 7–8, *Modified Nodal Analysis* bakal ngasilin $G*v = i$ buat keadaan tertentu, lalu buat *transient* bakal jadi $(G + C/dt)*v_{n+1} = i_{n+1} + C/dt*v_n$ (mirip pola ini).
- **Beda dari event-driven**: Di `tw-core/tw-sim` sekarang kita lompat antar event. Di circuit solver kita bakal "melangkah" waktu dengan $\Delta t$ dan hitung nilai kontinu.
- **Nunjukin "constraint muncul"**: Kapasitor bikin respon jadi lambat/eksponensial. Efek kayak *rise time*, *charging delay* ini yang nantinya bisa muncul kalo kita bangun dari komponen fisik, bukan kita set manual.
- **Validasi `tw-num`**: Test ini nunjukin crate numerik kita udah cukup buat dipake sebagai *linear solver* di dalam time-stepping. Ini exactly yang dibutuhkan buat Phase 6.

## Glossary Singkat

| Istilah | Penjelasan |
|---|---|
| **MNA** | *Modified Nodal Analysis* - cara sistematis ubah rangkaian jadi $A v = b$ |
| **KCL** | *Kirchhoff's Current Law* - jumlah arus masuk = keluar di node |
| **Steady-state/DC** | Kondisi "stabil", nggak ada perubahan waktu (turunan nol) |
| **Transient** | Proses transisi/perubahan seiring waktu |
| **Backward Euler** | Metode integrasi implisit (harus solve linier tiap step), stabil buat banyak kasus RC/RL |
| **Time constant $\tau=RC$** | Ukuran seberapa cepat capacitor ngisi/empty |
| **Node voltage** | Tegangan di suatu titik (node) relatif ke ground |
| **Conductance $G=1/R$** | Kebalikan resistansi, lebih enak dipake di MNA |
