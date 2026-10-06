# 1. Desain Arsitektur

## A. Core Simulation Engine

Fondasi paling bawah dan domain-neutral.

```text
Core Engine
├── Time
├── State
├── Event
├── Signal
├── Connection
├── Component
├── Composition
├── Resource
└── Observation
```

### Tugasnya

* menjalankan simulation time
* mengelola perubahan state
* menjadwalkan event
* mengirim signal
* menghubungkan component
* mengelola resource dan contention
* menggabungkan component menjadi component yang lebih besar
* melakukan observasi/tracing

**Tidak tahu tentang CPU, RAM, NAND, OS, server, dll.**

---

# 2. Construction System

Lapisan yang memungkinkan user membuat teknologi.

```text
Construction API
├── define component
├── define ports
├── define state
├── connect components
├── compose components
├── define behavior
├── define resource
└── expose interface
```

Contoh:

```text
User
 │
 ▼
create transistor
 │
 ▼
connect transistor
 │
 ▼
create NAND
```

Engine tidak memiliki `NAND`.

User yang membuatnya.

---

# 3. Composition Model

Semua teknologi dibangun secara hierarkis:

```text
Primitive
   ↓
Component
   ↓
Composite Component
   ↓
System
   ↓
Machine
   ↓
Network
   ↓
World
```

Contoh:

```text
Transistor
   ↓
CMOS
   ↓
NAND
   ↓
Adder
   ↓
ALU
   ↓
CPU
   ↓
Computer
```

Prinsip penting:

> **Complexity comes from composition, not hardcoded domain objects.**

---

# 4. Multi-Fidelity System

Satu component dapat mempunyai beberapa implementasi.

```text
NAND
├── transistor-level
├── circuit-level
├── gate-level
└── behavioral
```

Interface tetap sama.

```text
A ──┐
    ├── NAND ── Y
B ──┘
```

Tetapi implementasinya dapat berbeda.

Ini menjadi mekanisme utama untuk mengatasi trade-off:

```text
Fidelity ↑
   ↓
Computation ↑
   ↓
Scale ↓
```

---

# 5. Derived Abstraction

Model detail dapat digunakan untuk menghasilkan model abstrak.

```text
Detailed Model
      │
      ▼
Simulation
      │
      ▼
Measurement
      │
      ▼
Derived Behavior
      │
      ▼
Abstract Model
```

Misalnya transistor NAND menghasilkan:

```text
logic behavior
propagation delay
energy
timing constraints
```

yang kemudian menjadi model NAND behavioral.

Jadi abstraction bukan sekadar:

```text
"anggap NAND bekerja seperti ini"
```

tetapi:

```text
"perilaku ini diperoleh dari model yang lebih detail"
```

---

# 6. Refinement System

Kebalikannya juga harus bisa dilakukan.

```text
Abstract Computer
       │
       ▼
   refine CPU
       │
       ▼
   refine ALU
       │
       ▼
   refine Gate
       │
       ▼
 refine Transistor
```

Tidak seluruh world harus direfine.

Misalnya:

```text
1,000,000 computers

999,999 → abstract
1        → transistor-level
```

Ini memungkinkan skala besar sekaligus investigasi detail.

---

# 7. Resource System

Resource juga harus generik.

```text
Resource
├── capacity
├── allocation
├── availability
├── contention
├── queue
└── release
```

Jangan langsung membuat:

```text
RAM
CPU
Disk
Network bandwidth
```

sebagai primitive.

User bisa membangun RAM dari:

```text
cell
→ array
→ addressing
→ memory system
```

---

# 8. Emergence

Behavior harus muncul dari mekanisme yang dibangun.

Bukan:

```text
if RAM_full:
    server_slow()
```

Tetapi:

```text
Memory allocation
      ↓
No available memory
      ↓
Memory management
      ↓
Paging
      ↓
Storage I/O
      ↓
Queue
      ↓
Process blocked
      ↓
Scheduler
      ↓
Latency
```

Jadi engine tidak perlu tahu konsep **"server lambat"**.

---

# 9. Causal Tracing

Harus bisa melakukan:

### Top-down

```text
Application slow
      ↓
OS
      ↓
CPU
      ↓
ALU
      ↓
Gate
      ↓
Transistor
```

### Bottom-up

```text
Transistor delay
      ↓
Gate delay
      ↓
ALU
      ↓
CPU
      ↓
OS
      ↓
Application
```

Ini penting karena salah satu tujuan project adalah memahami **bagaimana behavior pada level tinggi berasal dari mekanisme level rendah**.

---

# 10. Fase Implementasi

Saya akan membaginya menjadi sekitar **9 fase**.

## Phase 0 — Simulation Kernel

Bangun:

```text
Time
State
Event
Signal
Connection
Component
Composition
Observation
```

Target:

> Bisa membuat dan menjalankan sistem generik.

**Belum ada transistor, NAND, CPU, atau RAM.**

---

## Phase 1 — Construction Framework

Bangun API untuk:

```text
create component
define port
define state
connect
compose
simulate
observe
```

Target:

> User dapat membuat component sendiri menggunakan API yang sama dengan yang digunakan engine.

---

## Phase 2 — Minimal Physical Foundation

Mulai membuat primitive fisik yang dibutuhkan.

Contoh awal:

```text
Voltage
Current
Resistance
Capacitance
Switching
```

Tidak perlu langsung membuat simulator semiconductor yang sangat kompleks.

Target:

> Ada foundation yang cukup untuk membuat switching device.

---

## Phase 3 — Transistor

**Transistor dibuat sebagai user-level technology**, bukan sebagai special engine object.

Kemudian gunakan transistor untuk membuat:

```text
Inverter
NAND
NOR
```

Milestone terpenting:

> **NAND berhasil dibuat tanpa NAND primitive di engine.**

---

## Phase 4 — Digital Composition

Bangun secara bertahap:

```text
NAND
 ↓
XOR
 ↓
Half Adder
 ↓
Full Adder
 ↓
Adder
 ↓
Multiplexer
 ↓
Register
 ↓
ALU
```

Di sini mulai diuji apakah composition benar-benar scalable.

---

## Phase 5 — Abstraction & Fidelity

Setelah NAND/ALU bekerja pada level detail:

```text
Detailed NAND
      ↓
Measure
      ↓
Abstract NAND
```

Kemudian:

```text
Detailed ALU
      ↓
Abstract ALU
```

Implementasikan:

* abstraction
* validation
* fidelity selection
* refinement
* error budget

Ini adalah **fase penting untuk menyelesaikan trade-off komputasi**.

---

# 11. Phase 6 — CPU

Baru kemudian:

```text
ALU
+
Register
+
Control
+
Memory Interface
+
Clock
```

menjadi CPU.

Tetapi CPU tetap **user-created technology**.

Setelah berhasil:

```text
Detailed CPU
      ↓
CPU abstraction
```

sehingga CPU dapat digunakan dalam simulasi yang lebih besar.

---

# 12. Phase 7 — Computer

Bangun:

```text
CPU
Memory
Storage
I/O
```

dari component yang sudah tersedia.

Kemudian:

```text
Computer
      ↓
Computer abstraction
```

Target:

> Satu computer dapat menjalankan software sederhana tanpa seluruh sistem harus disimulasikan transistor-by-transistor.

---

# 13. Phase 8 — OS + Network

Kemudian user dapat membangun:

```text
OS
 ├── process
 ├── scheduler
 ├── memory management
 ├── filesystem
 └── drivers
```

dan:

```text
Network
 ├── link
 ├── packet
 ├── protocol
 ├── routing
 └── transport
```

Tidak perlu membuat `Server` sebagai primitive.

Server cukup merupakan:

```text
Computer
+
OS
+
Network
+
Application
```

---

# 14. Phase 9 — Tech World

Barulah semuanya dapat membentuk:

```text
                 WORLD
                   │
       ┌───────────┼───────────┐
       │           │           │
    Computer    Network     Storage
       │
      CPU
       │
      ALU
       │
     Gates
       │
      CMOS
       │
   Transistors
```

Dengan:

```text
multi-fidelity
+
refinement
+
causal tracing
+
resource contention
+
event-driven simulation
```

maka user dapat membuat teknologi dan world mereka sendiri.

---

# 15. Prioritas Fase

Kalau dibuat lebih praktis, saya akan memprioritaskan:

```text
                    ┌────────────────────┐
                    │   Tech World       │
                    └─────────┬──────────┘
                              │
                    Phase 9: World
                              │
                    Phase 8: OS/Network
                              │
                    Phase 7: Computer
                              │
                    Phase 6: CPU
                              │
                    Phase 5: Fidelity
                              │
                    Phase 4: Digital
                              │
                    Phase 3: Transistor
                              │
                    Phase 2: Physics
                              │
                    Phase 1: Construction
                              │
                    Phase 0: Simulation
```

Tetapi **Phase 5 sangat penting**. Jangan terlalu cepat mengejar CPU.

Karena pertanyaan terbesar project Anda sebenarnya bukan:

> "Bisakah kita membuat CPU?"

Melainkan:

> **"Bisakah kita membuat sesuatu secara bottom-up, kemudian mengabstraksikannya tanpa kehilangan behavior penting, lalu menggunakan abstraction tersebut untuk membangun sistem yang jauh lebih besar?"**

Kalau itu berhasil pada:

```text
Transistor → NAND → ALU
```

maka fondasi untuk:

```text
CPU → Computer → OS → Network → World
```

sudah terbukti.
