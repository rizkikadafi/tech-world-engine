Untuk **General Simulation Core Phase 0–5**, menurut saya jangan mengambil dasar dari teknologi seperti CPU simulator atau ngspice secara langsung. Dasarnya lebih tepat diambil dari **teori discrete-event simulation, state-transition systems, component-based modeling, resource/queueing theory, dan causal/event tracing**.

Tujuannya adalah membuat sebuah **substrate simulasi yang domain-agnostic**.

```text
                 GENERAL SIMULATION CORE
                         │
        ┌────────────────┼────────────────┐
        │                │                │
      TIME             STATE            EVENT
        │                │                │
        └────────────────┼────────────────┘
                         ↓
                    COMPONENT
                         │
              ┌──────────┴──────────┐
              ↓                     ↓
          RESOURCE              CONNECTION
              │                     │
              └──────────┬──────────┘
                         ↓
                  OBSERVATION
                         ↓
                  CAUSAL TRACE
```

## 1. Dasar utamanya: State Transition System

Fondasi paling umum yang bisa digunakan adalah konsep:

> **Sistem memiliki state, kemudian event/input menyebabkan state transition.**

Secara abstrak:

$$
S_{t+1} = F(S_t, E_t)
$$

di mana:

* `S` = state sistem
* `E` = event
* `F` = aturan transisi
* `t` = simulation time

Misalnya:

```text
State:
    switch = OFF
    voltage = 0V

Event:
    switch_pressed

Transition:
    switch = ON
    voltage = 5V
```

Engine tidak perlu tahu bahwa itu "switch".

Engine hanya tahu:

```text
current state
      +
event
      ↓
state transition
```

Ini adalah fondasi yang sangat kuat karena nantinya:

```text
Switch
Transistor
NAND
CPU
Process
Network
Server
Database
```

semuanya bisa direpresentasikan sebagai sistem yang mengalami **state transition**.

---

# 2. Time

Time adalah dimensi tempat perubahan terjadi.

Tetapi ada satu keputusan penting:

### Jangan langsung mengasumsikan time = real time

Engine sebaiknya memiliki **simulation time** sendiri.

```text
Real World
    │
    │ host computation
    ↓
Simulation

t = 0
t = 1 ns
t = 2 ns
t = 5 ns
...
```

Jadi:

```rust
SimulationTime
```

adalah konsep internal engine.

Ada dua paradigma besar yang bisa dijadikan dasar:

### A. Discrete Event Simulation

Waktu melompat dari event ke event.

```text
t=0
  │
  ├── event A
  ↓
t=10
  │
  ├── event B
  ↓
t=13
  │
  └── event C
```

Tidak perlu menghitung setiap `t=1,2,3,4...`.

Ini sangat cocok untuk general engine.

Dasar teorinya bisa melihat literatur **Discrete Event Simulation (DES)** dan event scheduling.

### B. Continuous / numerical simulation

Misalnya electrical simulation:

```text
t = 0
t = 1 ns
t = 2 ns
t = 3 ns
...
```

dengan numerical integration.

Ini nanti masuk Phase 6+.

Jadi General Core sebaiknya tidak mengimplementasikan physics timestep secara langsung.

Ia cukup menyediakan abstraksi:

```text
SimulationTime
Scheduler
EventQueue
```

Kemudian electrical solver dapat menggunakan mekanisme waktunya sendiri.

---

# 3. State

State adalah **informasi yang menentukan kondisi sistem pada suatu simulation time**.

Contoh:

```text
Component A

state:
    enabled = true
    temperature = 40
    voltage = 5
```

Tetapi jangan membuat:

```text
enum State {
    CPUState,
    RAMState,
    TransistorState
}
```

Karena itu membuat core mengetahui domain.

Lebih baik:

```text
State
├── scalar
├── vector
├── structured data
└── domain-defined variables
```

Secara konseptual:

```text
Component
     │
     └── State
           ├── variable A
           ├── variable B
           └── variable C
```

State juga sebaiknya memiliki konsep **snapshot**:

```text
t=0 ── snapshot S0
t=1 ── snapshot S1
t=2 ── snapshot S2
```

Ini nantinya sangat berguna untuk:

* debugging
* replay
* time travel
* checkpoint
* causal analysis
* refinement

---

# 4. Event

Event adalah sesuatu yang menyebabkan atau merepresentasikan perubahan pada sistem.

Model sederhananya:

```text
Event
├── timestamp
├── source
├── target
├── payload
└── metadata
```

Contoh:

```text
Event {
    time: 10ns,
    source: switch_1,
    target: gate_1,
    payload: signal_changed
}
```

Namun ada konsep penting:

> **Event bukan berarti semua perubahan harus menjadi event.**

Untuk electrical simulation misalnya, perubahan voltage mungkin merupakan hasil solver, bukan event discrete biasa.

Karena itu General Core perlu membedakan:

```text
Discrete Event
```

dengan:

```text
Continuous/Numerical State Evolution
```

Ini akan membuat arsitektur jauh lebih fleksibel.

---

# 5. Component

Component adalah salah satu bagian terpenting.

Saya akan mendasarkannya pada konsep **composable component / hierarchical modeling**.

Secara abstrak:

```text
Component
├── Interface
│   ├── Input
│   └── Output
│
├── State
│
├── Behavior
│
├── Connections
│
└── Subcomponents
```

Contohnya component sederhana:

```text
A ─────┐
       │
       ▼
    Component
       │
       ▼
       Y
```

Kemudian component dapat disusun:

```text
Component A
      │
      ▼
Component B
      │
      ▼
Component C
```

Yang penting adalah **composite component menggunakan mekanisme yang sama dengan primitive component**.

Contoh:

```text
MOSFET
  ↓
CMOS Inverter
  ↓
NAND
  ↓
ALU
  ↓
CPU
```

Engine tidak perlu mempunyai:

```text
class CPU
class ALU
class NAND
```

Core hanya tahu:

```text
Component
Connection
State
Behavior
```

Ini adalah prinsip **recursive composition**.

---

# 6. Connection

Walaupun tidak masuk daftar Anda, saya justru menganggap `Connection` sebagai primitive penting.

Karena tanpa connection:

```text
Component A
Component B
```

tidak bisa menjadi sistem.

General abstraction:

```text
Connection
├── source
├── destination
├── interface/port
└── propagated value/event
```

Contohnya:

```text
A.output
    │
    │ Connection
    ↓
B.input
```

Kemudian pada transistor:

```text
MOSFET.D ─── Node
MOSFET.G ─── Node
MOSFET.S ─── Node
```

Pada komputer:

```text
CPU ─── Bus ─── Memory
```

Pada network:

```text
Server ─── Link ─── Router
```

Mekanismenya sama.

---

# 7. Resource

Ini menarik karena bisa menghubungkan simulasi fisik dengan simulasi komputer.

Dasarnya dapat diambil dari **resource allocation + queueing theory**.

General resource:

```text
Resource
├── Capacity
├── Availability
├── Allocation
├── Release
├── Queue
├── Contention
└── Delay
```

Contoh sederhana:

```text
Resource capacity = 2

Request A ──┐
Request B ──┼──> Resource
Request C ──┘
```

A dan B mendapatkan resource.

C:

```text
waiting
```

Kemudian:

```text
A releases
     ↓
C gets resource
```

Tidak ada aturan:

```text
if RAM_full:
    process.slow()
```

Sebaliknya:

```text
Process
   ↓
Request resource
   ↓
Resource unavailable
   ↓
Queue
   ↓
Waiting
   ↓
Latency
```

**Perilaku tingkat tinggi muncul dari mekanisme resource.**

Ini sangat sesuai dengan filosofi Tech World.

---

# 8. Causal Tracing

Ini bagian yang menurut saya paling menarik untuk proyek Anda.

Dasarnya dapat diambil dari kombinasi:

* event provenance
* dependency graphs
* execution tracing
* causal graphs
* data lineage

Jangan hanya menyimpan:

```text
Event happened
```

tetapi:

```text
Event A
  caused
    ↓
State change B
  caused
    ↓
Event C
  caused
    ↓
State change D
```

Sehingga engine memiliki **causal graph**.

Misalnya:

```text
Input changed
      │
      ▼
Gate input changed
      │
      ▼
MOSFET current changed
      │
      ▼
Node voltage changed
      │
      ▼
Output changed
```

Kemudian user bisa bertanya:

```text
Why did output change?
```

dan engine melakukan traversal:

```text
Output change
      ↑
Node voltage change
      ↑
Current change
      ↑
MOSFET state
      ↑
Input change
```

Ini berbeda dengan sekadar logging.

### Logging

```text
10.0 input changed
10.1 transistor updated
10.2 voltage changed
10.3 output changed
```

### Causal tracing

```text
input_changed
      ↓ caused
transistor_current_changed
      ↓ caused
node_voltage_changed
      ↓ caused
output_changed
```

Yang kedua jauh lebih berguna untuk Tech World.

---

# 9. Jadi apa dasar teoritis Phase 0–5?

Saya akan memetakannya seperti ini:

| Tech World     | Dasar yang bisa dipelajari                      |
| -------------- | ----------------------------------------------- |
| Time           | Discrete-Event Simulation, simulation clock     |
| State          | State-transition systems, state-space modeling  |
| Event          | Discrete-event simulation, event scheduling     |
| Component      | Component-based modeling, hierarchical modeling |
| Connection     | Graph theory, ports/interfaces                  |
| Resource       | Resource allocation, queueing theory            |
| Scheduler      | Event scheduling algorithms                     |
| Observation    | Instrumentation, metrics, monitoring            |
| Causal Tracing | Provenance, dependency graph, causal graph      |
| Snapshot       | Checkpointing, state persistence                |
| Composition    | Hierarchical/decomposable systems               |

Jadi **General Simulation Core bukan sesuatu yang perlu Anda temukan dalam satu framework tertentu**.

Lebih tepatnya ia merupakan kombinasi beberapa disiplin.

---

# 10. Ada satu konsep yang menyatukan semuanya

Menurut saya, untuk Tech World konsep paling fundamental adalah:

```text
ENTITY
  │
  ├── has STATE
  │
  ├── has INTERFACE
  │
  ├── connects to other entities
  │
  ├── consumes/produces RESOURCES
  │
  └── undergoes STATE TRANSITIONS
             │
             ▲
             │
           EVENT
             │
             ▼
           TIME
```

Lalu engine mengamati:

```text
State Transition
       ↓
Observation
       ↓
Causal Graph
```

Sehingga core-nya kira-kira:

```text
                 SIMULATION
                     │
             ┌───────┴───────┐
             │               │
           TIME            STATE
             │               │
             └───────┬───────┘
                     │
                   EVENT
                     │
                     ▼
                 COMPONENT
                     │
              ┌──────┴──────┐
              │             │
         CONNECTION      RESOURCE
              │             │
              └──────┬──────┘
                     ↓
                TRANSITION
                     ↓
                 OBSERVE
                     ↓
              CAUSAL TRACE
```

Dan yang paling penting:

> **General Simulation Core sebaiknya tidak mengetahui apa yang sedang disimulasikan.**

Ia tidak tahu:

```text
transistor
CPU
RAM
server
network
process
database
```

Ia hanya tahu:

```text
time
state
event
component
connection
resource
transition
observation
causality
```

---

## Dari mana sebaiknya mulai membaca?

Kalau tujuan Anda adalah **membangun Phase 0–5**, saya akan memprioritaskan **Discrete-Event Simulation** sebagai pintu masuk, lalu baru memperluas ke component/resource/causality.

Urutannya:

```text
1. State Transition System
          ↓
2. Discrete Event Simulation
          ↓
3. Event Scheduling
          ↓
4. Hierarchical / Component Modeling
          ↓
5. Resource & Queueing
          ↓
6. Event Provenance / Causal Graph
```

Kemudian baru:

```text
General Simulation Core
          ↓
Numerical Simulation
          ↓
Circuit Simulation
          ↓
MOSFET
```

Dengan begitu Phase 0–5 benar-benar menjadi **fondasi independen dari electrical simulation**, bukan sekadar abstraksi yang dibuat khusus agar nanti bisa menjalankan transistor.

Kalau diringkas dalam satu kalimat:

> **Dasar General Simulation Core adalah state-transition + event scheduling + hierarchical composition + resource modeling + causal provenance.**

Itulah fondasi yang menurut saya paling cocok untuk ambisi Tech World sebagai **general-purpose simulation engine**.
