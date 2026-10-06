```text
tech-world/
├── Cargo.toml
├── Cargo.lock
│
├── crates/
│   ├── tw-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       │
│   │       ├── time/
│   │       │   ├── mod.rs
│   │       │   ├── simulation_time.rs
│   │       │   └── duration.rs
│   │       │
│   │       ├── state/
│   │       │   ├── mod.rs
│   │       │   ├── state.rs
│   │       │   └── snapshot.rs
│   │       │
│   │       ├── event/
│   │       │   ├── mod.rs
│   │       │   ├── event.rs
│   │       │   ├── queue.rs
│   │       │   └── scheduler.rs
│   │       │
│   │       ├── component/
│   │       │   ├── mod.rs
│   │       │   ├── component.rs
│   │       │   ├── port.rs
│   │       │   └── hierarchy.rs
│   │       │
│   │       ├── connection/
│   │       │   ├── mod.rs
│   │       │   └── connection.rs
│   │       │
│   │       ├── resource/
│   │       │   ├── mod.rs
│   │       │   ├── resource.rs
│   │       │   ├── allocation.rs
│   │       │   └── queue.rs
│   │       │
│   │       ├── observation/
│   │       │   ├── mod.rs
│   │       │   ├── observer.rs
│   │       │   └── metric.rs
│   │       │
│   │       └── tracing/
│   │           ├── mod.rs
│   │           ├── trace.rs
│   │           ├── causal_graph.rs
│   │           └── provenance.rs
│   │
│   ├── tw-sim/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── simulation.rs
│   │       ├── engine.rs
│   │       └── runner.rs
│   │
│   └── tw-cli/
│       └── src/
│           └── main.rs
│
├── models/
│   └── ...
│
├── examples/
│   ├── counter/
│   ├── traffic_light/
│   └── resource_contention/
│
├── tests/
│   └── ...
│
├── benchmarks/
│   └── ...
│
└── docs/
    ├── architecture/
    ├── concepts/
    └── decisions/
```

Namun ada satu perubahan penting dari struktur sebelumnya: **saya akan memisahkan `tw-core` dan `tw-sim`.**

## 1. `tw-core`: primitive simulation

`tw-core` berisi **konsep paling fundamental**, tetapi tidak menjalankan simulation loop.

```text
tw-core
│
├── Time
├── State
├── Event
├── Component
├── Port
├── Connection
├── Resource
├── Observation
└── Causal Trace
```

Secara dependency:

```text
                 tw-core
                    │
       ┌────────────┼────────────┐
       ↓            ↓            ↓
     Time         State        Event
       │            │            │
       └────────────┼────────────┘
                    ↓
                Component
                    │
          ┌─────────┴─────────┐
          ↓                   ↓
      Connection           Resource
          │                   │
          └─────────┬─────────┘
                    ↓
              Observation
                    ↓
              Causal Trace
```

Ini adalah **building blocks**.

---

# 2. `tw-sim`: mesin yang menjalankan semuanya

Kemudian:

```text
tw-sim
│
├── Simulation
├── SimulationEngine
├── Scheduler
├── EventLoop
└── Runner
```

Contohnya secara konseptual:

```rust
pub struct Simulation {
    clock: SimulationTime,
    event_queue: EventQueue,
    components: ComponentRegistry,
}
```

Lalu:

```rust
impl Simulation {
    pub fn run(&mut self) {
        while let Some(event) = self.event_queue.pop() {
            self.clock = event.time();

            // process event
            // update state
            // schedule new events
            // record causal information
        }
    }
}
```

Jadi:

```text
tw-core
   │
   │ primitives
   ▼
tw-sim
   │
   │ execution
   ▼
Simulation
```

Ini boundary yang bagus.

---

# 3. `tw-cli`

CLI hanya menjadi interface untuk menjalankan engine.

Misalnya:

```bash
tw run examples/counter
```

atau:

```bash
tw run examples/resource-contention
```

atau nanti:

```bash
tw inspect component counter
```

CLI **tidak boleh menjadi tempat logic simulation**.

```text
CLI
 │
 ▼
tw-sim
 │
 ▼
tw-core
```

Bukan:

```text
CLI
 ├── event logic
 ├── state logic
 ├── scheduler
 └── simulation logic
```

---

# 4. Time

Saya akan membuatnya sangat sederhana terlebih dahulu.

```rust
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct SimulationTime {
    ticks: u64,
}
```

Kenapa `ticks`, bukan langsung `f64`?

Karena General Core sebaiknya tidak memaksakan:

```text
nanoseconds
seconds
milliseconds
```

Misalnya:

```text
1 tick
10 ticks
100 ticks
```

Kemudian domain tertentu dapat menentukan interpretasinya.

Nanti electrical solver bisa memiliki:

```text
1 tick = 1 ps
```

sedangkan simulation komputer:

```text
1 tick = 1 ns
```

Tetapi ini perlu dirancang lebih matang ketika masuk numerical simulation.

---

# 5. Event

Misalnya:

```rust
pub struct Event {
    pub time: SimulationTime,
    pub source: ComponentId,
    pub target: ComponentId,
    pub payload: EventPayload,
}
```

Untuk tahap awal:

```rust
pub enum EventPayload {
    SignalChanged,
    WakeUp,
    ResourceReleased,
    Custom(Box<dyn Any>),
}
```

Tetapi saya **tidak akan buru-buru membuat enum seperti itu**.

Lebih generik:

```rust
pub struct Event {
    pub time: SimulationTime,
    pub source: ComponentId,
    pub target: ComponentId,
    pub payload: EventData,
}
```

dan `EventData` bisa dikembangkan kemudian.

---

# 6. Component

Ini kemungkinan menjadi abstraction paling penting.

Saya lebih menyarankan **trait minimal**:

```rust
pub trait Component {
    fn id(&self) -> ComponentId;

    fn handle_event(
        &mut self,
        event: &Event,
        ctx: &mut SimulationContext,
    );
}
```

Kemudian user dapat membuat:

```rust
struct Counter {
    id: ComponentId,
    value: u64,
}
```

dan:

```rust
impl Component for Counter {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn handle_event(
        &mut self,
        event: &Event,
        ctx: &mut SimulationContext,
    ) {
        self.value += 1;

        // schedule next event
    }
}
```

Engine tidak tahu bahwa `Counter` adalah counter.

Ia hanya tahu:

```text
Component
```

---

# 7. Port

Component kemudian punya interface.

```rust
pub struct PortId(pub u64);

pub struct Port {
    pub id: PortId,
    pub direction: PortDirection,
}
```

Misalnya:

```text
       Component A
      ┌─────────────┐
 ---> │ input       │
      │             │
 <--- │ output      │
      └─────────────┘
```

Nanti konsep ini akan sangat penting untuk:

```text
MOSFET
   G
   D
   S

NAND
   A
   B
   Y

CPU
   clock
   bus
   interrupt
```

Tetapi semuanya tetap menggunakan:

```text
Port
```

---

# 8. Connection

```rust
pub struct Connection {
    pub source: PortRef,
    pub target: PortRef,
}
```

Kemudian:

```text
A.output
    │
    ▼
Connection
    │
    ▼
B.input
```

Dengan ini composition graph mulai terbentuk.

---

# 9. Resource

Resource juga sebaiknya trait/abstraction generik.

```rust
pub struct ResourceId(pub u64);

pub struct Resource {
    pub id: ResourceId,
    pub capacity: u64,
    pub available: u64,
}
```

Operasi:

```rust
impl Resource {
    pub fn allocate(&mut self, amount: u64) -> AllocationResult {
        ...
    }

    pub fn release(&mut self, amount: u64) {
        ...
    }
}
```

Kemudian resource contention:

```text
Request A ──┐
Request B ──┼──> Resource
Request C ──┘
                 │
                 ↓
              Queue
```

Yang penting:

**Resource tidak tahu apakah dia RAM, CPU, bandwidth, storage, atau energi.**

---

# 10. Causal tracing

Saya akan membuat ini sebagai **graph**, bukan sekadar log.

```rust
pub struct TraceNode {
    pub id: TraceId,
    pub time: SimulationTime,
    pub source: TraceSource,
}

pub struct CausalEdge {
    pub from: TraceId,
    pub to: TraceId,
    pub relation: CausalRelation,
}
```

Contoh graph:

```text
InputChanged
     │
     │ caused
     ▼
StateChanged
     │
     │ caused
     ▼
EventScheduled
     │
     │ caused
     ▼
OutputChanged
```

Sehingga nantinya:

```rust
trace.explain(output_change)
```

dapat menghasilkan:

```text
OutputChanged
  ← StateChanged
  ← Event
  ← InputChanged
```

Ini bisa menjadi salah satu fitur pembeda Tech World.

---

# 11. Observation sebaiknya dipisahkan dari tracing

Jangan campur:

```text
Observation
```

dengan:

```text
Causality
```

Observation menjawab:

> **Apa yang terjadi?**

Contoh:

```text
voltage = 3.2 V
temperature = 45 C
queue_length = 12
```

Causal tracing menjawab:

> **Mengapa hal itu terjadi?**

```text
queue_length increased
    ↑
resource unavailable
    ↑
capacity exhausted
    ↑
allocation A
```

Jadi:

```text
             Simulation
                 │
        ┌────────┴────────┐
        ↓                 ↓
   Observation         Causality
        │                 │
      "WHAT"            "WHY"
```

Ini sangat penting secara arsitektur.

---

# 12. Contoh keseluruhan

Misalnya kita membuat sistem sederhana:

```text
Generator
    │
    ▼
Queue
    │
    ▼
Processor
```

Tidak ada satupun yang harus menjadi primitive engine.

```rust
struct Generator;
struct Queue;
struct Processor;
```

Engine hanya melihat:

```text
Component
Component
Component
```

Dengan:

```text
Connection
Event
State
Resource
```

Simulation:

```text
t=0
Generator creates job
       ↓
Event(JobCreated)

t=1
Queue receives job
       ↓
State(queue_length = 1)

t=2
Processor requests resource
       ↓
Resource allocated

t=3
Processor completes job
       ↓
Resource released
       ↓
Queue decreases
```

Causal graph:

```text
JobCreated
    ↓
QueueLength +1
    ↓
ProcessorRequest
    ↓
ResourceAllocated
    ↓
ProcessingCompleted
    ↓
ResourceReleased
    ↓
QueueLength -1
```

**Tidak ada rule khusus tentang "job", "queue", atau "processor" di core.**

---

# 13. Dependency akhirnya

Saya akan menjaga dependency kira-kira seperti ini:

```text
                tw-cli
                  │
                  ▼
                tw-sim
                  │
                  ▼
                tw-core
```

Dan nanti:

```text
                    tw-cli
                      │
                      ▼
                   tw-sim
                      │
          ┌───────────┴───────────┐
          ▼                       ▼
      tw-core                domain models
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
                electrical      digital      network
```

Kemudian numerical:

```text
                  tw-sim
                    │
             ┌──────┴──────┐
             ▼             ▼
          tw-core       numerical
                            │
                            ▼
                         solver
                            │
                            ▼
                       electrical
```

**Core tidak bergantung pada electrical.**

Ini boundary yang menurut saya paling penting untuk dijaga sejak awal.

---

## Saya bahkan akan mulai lebih kecil

Untuk commit pertama, jangan buat seluruh struktur di atas.

Mulai:

```text
tech-world/
├── Cargo.toml
├── crates/
│   ├── tw-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── time.rs
│   │       ├── state.rs
│   │       ├── event.rs
│   │       └── component.rs
│   │
│   └── tw-sim/
│       └── src/
│           ├── lib.rs
│           └── simulation.rs
│
├── examples/
│   └── counter.rs
│
└── tests/
```

Lalu milestone pertama cukup:

```text
Component
    ↓
Event
    ↓
State
    ↓
Simulation Time
    ↓
Event Queue
    ↓
Simulation Loop
```

Kalau **Counter sederhana** sudah bisa berjalan melalui mekanisme tersebut tanpa ada pengetahuan khusus tentang `Counter` di engine, berarti fondasi Phase 0–1 Anda sudah berada di jalur yang benar.

Setelah itu baru tambahkan `Port → Connection → Composition`, lalu `Resource`, kemudian `Observation → Causal Trace`.
