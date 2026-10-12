# Time Model — Status, Keterbatasan, dan Opsi

Dokumen ini mencatat **keputusan & pertimbangan seputar model waktu** di engine,
supaya bisa dirujuk kembali (recall) saat masuk Phase 6+.

Ringkas: model waktu sekarang **tidak rusak**. Ia dirancang untuk *Discrete Event
Simulation* (DES) dan sudah tepat untuk logika digital. Yang perlu diputuskan nanti
(bukan sekarang) adalah bagaimana **mensinkronkan** waktu diskret dengan solver
kontinu untuk simulasi electrical/circuit.

---

## Ringkasan (TL;DR)

- Model sekarang: `SimulationTime { ticks: u64 }` — **integer, tanpa satuan**.
- Cocok untuk DES: deterministik, ordering eksak, lompat event sparse.
- **Belum cukup** untuk analog kontinu (butuh waktu fraksional + timestep adaptif).
- Dokumen desain sudah memutuskan: **core tidak mengimplementasikan physics
  timestep**; solver kontinu memakai mekanisme waktunya sendiri
  (`general-simulation-core.md` §2).
- **Rekomendasi:**
  1. Biarkan `u64` ticks untuk core.
  2. Solver analog mengurus waktunya sendiri di dalam komponen (**Opsi A**) —
     opsi paling kecil risikonya.
  3. Jika kelak butuh delay fraksional di dunia diskret → naikkan ke
     **fixed-point/rasional (Opsi B)**, bukan `f64`.
  4. **Hindari `f64` sebagai waktu core (Opsi C)** karena merusak `Ord`/`Eq`/`Hash`
     dan determinisme.
- **Kapan relevan?** Bukan Phase 6 (numerik). Relevan mulai **Phase 7–8**
  (representasi circuit & circuit solver).

---

## 1. Model saat ini

```rust
// crates/tw-core/src/time/simulation_time.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimulationTime { ticks: u64 }

impl SimulationTime {
    pub const ZERO: Self = Self { ticks: 0 };
    pub fn new(ticks: u64) -> Self;
    pub fn ticks(&self) -> u64;
    pub fn advance(&mut self, amount: u64);
}
```

Dipakai oleh:

- **Antrian event** (`BinaryHeap<ScheduledEvent>`), terurut berdasarkan
  **`(time, EventId)`** — `time` dulu, `id` sebagai tie-break.
- `Simulation::run_until(until)` berhenti saat `next_event.time > until`.
- `SimulationEngine::run_ticks(n)` memetakan ke `run_until(now + n)`.

---

## 2. Kenapa ini benar untuk DES

1. **Total ordering lengkap.** `Ord`/`Eq`/`Hash` tersedia, jadi `BinaryHeap`
   tidak pernah ambigu.
2. **Determinisme.** Tie-break `(time, EventId)` membuat dua event pada tick yang
   sama selalu diproses dengan urutan identik.
3. **Event sparse.** Waktu "melompat" dari event ke event; tidak menghitung
   `t = 1, 2, 3, …`.
4. **Tanpa masalah float.** Tidak ada NaN, tidak ada pembulatan yang mengubah
   urutan.

Untuk logika digital (counter, full-adder, resource-contention, disk-service)
model ini sudah benar apa adanya.

---

## 3. Keterbatasan

1. **Tidak ada waktu fraksional.** Propagation delay 12 ns tidak bisa dinyatakan
   jika 1 tick = 1 ns dan butuh 12, apalagi 0,3 ns.
2. **Tanpa satuan.** `tick` tidak berarti detik/nanodetik; konvensi harus
   disepakati per model.
3. **Solver kontinu butuh timestep adaptif** yang ditentukan *saat runtime* oleh
   error control — bukan langkah tetap seperti `tick`.
4. **Tidak ada titik sinkronisasi** antara event diskret dan dinamika kontinu.

---

## 4. Yang sudah diputuskan oleh dokumen desain

`general-simulation-core.md` §2 (Time) secara eksplisit:

> Core cukup menyediakan abstraksi (`SimulationTime`, `Scheduler`, `EventQueue`);
> electrical solver dapat menggunakan mekanisme waktunya sendiri.

Artinya core **sengaja tidak** mengimplementasikan physics timestep. Karena itu
keterbatasan di atas bukan penghalang untuk Phase 6 (numerical foundation), yang
sifatnya murni matematika dan tidak peduli waktu.

---

## 5. Opsi sinkronisasi (untuk Phase 7–8)

```text
Discrete timeline (u64 ticks)          Continuous solver (waktu sendiri)
        │                                         │
        ▼                                         ▼
   event A ──► event B ──► ...            t0 ─► t1 ─► t2 ...
                     ▲
                     └── titik sinkronisasi (sync point)
```

| Opsi | Cara kerja | Untung | Rugi |
| --- | --- | --- | --- |
| **A. Analog internal** | Solver kontinu jadi **sub-solver di dalam sebuah komponen**; engine hanya memajukan *sync point* per tick | core tidak berubah; komponen diskret tetap deterministik | koordinasi/resolusi antar-domain kasar |
| **B. Waktu fixed-point** | `SimulationTime` jadi bilangan **rasional/fixed-point** (mis. `u64` + penyebut tetap) | tetap `Ord`/`Eq`/`Hash`; bisa delay fraksional | presisi terbatas; harus pilih resolusi |
| **C. `f64` untuk analog** | Solver pakai `f64` sendiri, lapor ke engine di titik diskret | natural untuk fisika | `f64` **kehilangan `Ord`/`Eq`/`Hash`** → ordering heap & determinisme harus ditangani khusus (NaN, perbedaan lintas platform) |

> **Peringatan:** jangan buru-buru mengganti `SimulationTime` menjadi `f64`.
> Itu merusak total ordering, tie-break `BinaryHeap`, dan determinisme — padahal
> determinisme adalah salah satu invarian inti proyek.

---

## 6. Rekomendasi

1. **Sekarang:** pertahankan `u64` ticks. Benar untuk DES/digital.
2. **Phase 8:** coba **Opsi A** lebih dulu (analog mengurus waktu sendiri di dalam
   komponen). Paling kecil risikonya dan tidak menyentuh core.
3. **Jika muncul kebutuhan delay fraksional** di dunia diskret: naikkan ke
   **Opsi B** (fixed-point/rasional) — tetap menjaga ordering.
4. **Hindari Opsi C** untuk waktu core. `f64` hanya boleh hidup *di dalam* solver
   kontinu, bukan di timeline engine.
5. **Opsional (berjaga-jaga):** tambahkan abstraksi tipis `TimeBase`/`Timestamp`
   agar core tidak terkunci pada satu representasi — tetapi **jangan**
   implementasikan seluruh varian sekarang.

---

## 7. Kapan keputusan ini relevan

| Phase | Relevansi |
| --- | --- |
| Phase 6 — Numerical Foundation | **Tidak relevan.** Murni matematika; tidak peduli waktu. |
| Phase 7 — Circuit Representation | Mulai relevan: satuan & resolusi waktu untuk model electrical. |
| Phase 8 — Circuit Solver | **Titik keputusan**: transient integration, timestep control, sinkronisasi. |

**Trigger untuk meng-upgrade model waktu** (naik dari `u64` ke fixed-point):

- Muncul kebutuhan menyatakan delay **sub-tick** (mis. 0,3 ns) pada event diskret.
- Terjadi *aliasing* karena resolusi tick terlalu kasar untuk hasil yang diinginkan.
- Muncul skenario multi-fidelity yang mencampur behavioral (tick) dan transistor
  (ps/ns) pada timeline yang sama.

---

## 8. Pertanyaan terbuka (untuk recall)

1. Berapa **resolusi tick** default (1 tick = berapa detik)? Atau tetap tanpa
   satuan dan diserahkan ke model?
2. Apakah event **diskret** sendiri kelak butuh **delay fraksional**, atau cukup
   integer tick + solver analog internal?
3. Bagaimana persisnya **sync point** antara solver analog dan timeline diskret
   (setiap tick? atas permintaan? toleransi berbasis error)?
4. Untuk hasil analog yang reproducible, bagaimana kebijakan **determinisme
   `f64`** lintas platform (iterasi tetap, tanpa fast-math, kriteria konvergensi
   berbasis toleransi)?
5. Apakah perlu abstraksi `TimeBase` sekarang, atau tunggu sampai benar-benar
   dibutuhkan (YAGNI)?

---

## 9. Referensi

- `general-simulation-core.md` §2 — Time & dua paradigma (DES vs continuous).
- `engine-v1.md` Phase 6–8 — Numerical Foundation → Circuit Representation →
  Circuit Solver (MNA, Newton-Raphson, transient integration, timestep control).
- `docs/implementation.md` — status implementasi fondasi P0–P2.
- `crates/tw-core/src/time/simulation_time.rs` — definisi `SimulationTime`.
- `crates/tw-sim/src/simulation.rs` — antrian event & ordering `(time, EventId)`.
