# Membuat Komponen Sendiri

Dokumen ini menjelaskan cara membangun komponen di atas API publik `tw-core`
dan `tw-sim`. Engine tidak perlu tahu domain Anda: selama perilaku komponen
dapat dinyatakan sebagai **state + event → state transition + event baru**, ia
bisa berjalan di engine yang sama.

Contoh lengkap yang bisa dijalankan:

```bash
cargo run -p tw-sim --example custom_component
```

## Model mental

```text
Event masuk
    ↓
Component::handle_event
    ↓
ubah State + kembalikan Vec<Event>
    ↓
engine menjadwalkan event baru
```

Komponen **tidak** memiliki loop sendiri dan **tidak** memajukan waktu. Waktu
hanya bergerak saat engine mengeksekusi event.

## Komponen minimal

```rust
use tw_core::{
    Component, ComponentId, Event, Port, SimulationContext, State,
};

#[derive(Debug)]
struct Counter {
    id: ComponentId,
    name: String,
    ports: Vec<Port>,
    state: State,
    count: u64,
}

impl Component for Counter {
    fn id(&self) -> ComponentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn ports(&self) -> &[Port] { &self.ports }
    fn state(&self) -> &State { &self.state }

    fn handle_event(&mut self, event: &Event, _ctx: &mut SimulationContext) -> Vec<Event> {
        // Perilaku domain Anda ada di sini.
        self.count += 1;
        self.state.set("count", self.count);
        Vec::new()
    }
}
```

Yang wajib:

| Method | Fungsi |
| --- | --- |
| `id` | Identitas unik komponen di simulation. |
| `name` | Nama untuk log dan tracing. |
| `ports` | Daftar port yang bisa disambungkan. |
| `state` | State publik untuk observer/checkpoint. |
| `handle_event` | Transisi state dan event baru. |

`on_mount` dan `on_unmount` opsional; default-nya tidak melakukan apa-apa.

## Port dan koneksi

Port dideklarasikan sekali di konstruktor:

```rust
use tw_core::{Port, PortDirection, PortId};

const IN: PortId = PortId(1);
const OUT: PortId = PortId(2);

let ports = vec![
    Port::new(IN, "in", PortDirection::Input),
    Port::new(OUT, "out", PortDirection::Output),
];
```

Sambungkan output ke input komponen lain:

```rust
use tw_core::{ComponentId, Connection};

sim.register_connection(Connection::new(
    ComponentId(1), // sumber
    OUT,
    ComponentId(2), // tujuan
    PortId(1),
));
```

Engine mem-forward `EventPayload::PortSignal` dari port output ke semua
koneksi yang cocok. Koneksi yang tidak cocok **diam-diam tidak meneruskan
apa pun**, jadi panggil validator setelah wiring:

```rust
sim.validate_wiring().expect("wiring valid");
```

Validator melaporkan komponen/port yang tidak ada, arah port yang salah, dan
koneksi duplikat sekaligus. Validasi bersifat opsional dan tidak mengubah
perilaku runtime.

## Menjadwalkan event

Gunakan `Event::at`; engine akan mengganti `EventId` placeholder dengan id
otoritatif:

```rust
use tw_core::{Event, EventPayload, SimulationTime, StateValue};

vec![Event::at(
    SimulationTime::new(event.time.ticks() + 5),
    self.id,
    self.id,
    EventPayload::PortSignal {
        port_id: OUT,
        value: StateValue::from(42),
    },
)]
```

Aturan penting:

- Jangan mengarang `EventId`; itu dapat bertabrakan dengan id engine dan
  merusak determinisme.
- `EventPayload::Custom(String)` berguna untuk event internal komponen.
- Untuk event ke komponen lain, isi `target` dengan `ComponentId` tujuan.
- Saat mengembalikan `PortSignal`, isi `source` dengan `self.id`; engine
  mencocokkan `source` + `port_id` dengan koneksi yang terdaftar.
- `EventPayload::PortSignal` yang dikembalikan dari `handle_event` akan
  dirutekan lewat koneksi; event lain dikirim langsung ke `target`.

## State, observer, dan checkpoint

Simpan semua state yang ingin diamati di `State`:

```rust
self.state.set("total", self.total);
self.state.set("mode", "running");
```

Tipe yang tersedia: `Bool`, `Int`, `Float`, `Text`. Nilai bisa dibaca lewat
`Simulation::component_state(id)`.

Observer menerima setiap event dan snapshot saat checkpoint:

```rust
use tw_core::{Event, Observer, Snapshot};

#[derive(Debug)]
struct Logger;

impl Observer for Logger {
    fn on_event(&mut self, event: &Event) {
        println!("t={} {:?}", event.time.ticks(), event.payload);
    }

    fn on_snapshot(&mut self, snapshot: &Snapshot) {
        println!("checkpoint t={}", snapshot.time.ticks());
    }
}

sim.register_observer(Box::new(Logger));
```

`Simulation::checkpoint()` menyimpan `State` setiap komponen dan state resource.
Saat ini restore otomatis tersedia untuk resource (`restore_resources`).
Untuk komponen, implementasikan `fn restore_state(&mut self, state: &State) -> bool` – salin nilai snapshot ke `self.state` dan kembalikan `true`. Panggil `sim.restore(&checkpoint)` untuk memulihkan kedua resource dan komponen yang mendukung restore.

## Lifecycle

```rust
fn on_mount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
    self.state.set("mounted_at", ctx.time().ticks() as i64);
    Vec::new()
}

fn on_unmount(&mut self, ctx: &mut SimulationContext) -> Vec<Event> {
    self.state.set("unmounted_at", ctx.time().ticks() as i64);
    Vec::new()
}
```

- `on_mount` dipanggil sekali saat `register_component`.
- `on_unmount` dipanggil saat `Simulation::shutdown`.
- Event yang dikembalikan dari kedua hook akan dijadwalkan engine.
- Untuk komponen top-level, event mount yang menargetkan diri sendiri akan
  diproses normal pada tick berikutnya.

## Resource

Resource adalah primitive engine untuk kapasitas, antrean, dan latency:

```rust
use tw_core::{Amount, Event, EventPayload, Resource, ResourceId};

sim.register_resource(
    Resource::new(ResourceId(1), "worker", Amount::Discrete(2))
        .with_service_time(5),
);

sim.schedule_event(Event::at(
    SimulationTime::new(0),
    self.id,
    self.id,
    EventPayload::ResourceRequested {
        resource_id: ResourceId(1),
        amount: Amount::Discrete(1),
        requester: self.id,
    },
));
```

Engine akan mengirim `ResourceGranted` atau `ResourceDenied`, dan otomatis
mengirim `ResourceReleased` setelah `service_time`. Komponen Anda hanya perlu
menangani payload tersebut.

## Composite component

`CompositeComponent` menyusun komponen dari subkomponen:

```rust
use tw_core::{CompositeComponent, Connection, PortBinding};

let mut adder = CompositeComponent::new(ComponentId(10), "adder");
adder.add_port(/* ... */);
adder.add_subcomponent(Box::new(/* komponen lain */));
adder.bind_port(PortBinding::new(external_port, sub_id, sub_port));
adder.add_internal_connection(Connection::new(sub_a, out, sub_b, in));

adder.validate().expect("composite wiring valid");
```

Composite merutekan `PortSignal` dari input eksternal ke subkomponen, antar
subkomponen, dan dari subkomponen ke output eksternal. Untuk perilaku yang
lebih kaya (misalnya event `Custom` yang dijadwalkan subkomponen), gunakan
komponen top-level atau tangani routing tersebut di dalam composite Anda.

## Tracing

Tracing diatur engine, bukan komponen:

```rust
use tw_core::TracingMode;

sim.set_tracing_mode(TracingMode::Abstract);
```

- `Off` tidak menyimpan node; paling murah.
- `Abstract` mencatat satu node per event.
- `Hierarchical` juga mencatat routing internal composite.

Komponen cukup mengembalikan event dengan `cause` yang benar; engine akan
menghubungkan cause tersebut ke trace saat event dieksekusi.

## Menguji komponen Anda

Integration test adalah cara terbaik karena hanya memakai API publik:

```rust
#[test]
fn doubler_routes_output_to_sink() {
    let mut sim = Simulation::new();
    sim.register_component(Box::new(Doubler::new()));
    sim.register_component(Box::new(Sink::new()));
    sim.register_connection(Connection::new(
        DOUBLER_ID, OUTPUT, SINK_ID, SINK_INPUT,
    ));
    assert_eq!(sim.validate_wiring(), Ok(()));

    let mut engine = SimulationEngine::new(sim);
    engine.run_ticks(5);

    let sink = engine.simulation().component_state(SINK_ID).unwrap();
    assert_eq!(sink.get_int("last"), Some(8));
}
```

Checklist:

- [ ] Port input/output dideklarasikan dan arahnya benar.
- [ ] `validate_wiring()` dipanggil setelah semua koneksi terdaftar.
- [ ] `handle_event` mengabaikan payload/port yang tidak relevan.
- [ ] Event baru memakai `Event::at`, bukan `EventId` buatan sendiri.
- [ ] State yang ingin diamati disimpan di `State`.
- [ ] Ada test untuk routing, lifecycle, dan determinisme (jalankan dua kali).
- [ ] Mount event yang dijadwalkan sudah diproses sebelum mengukur hasil.

## Pitfall umum

1. **Port id salah** — sinyal hilang tanpa error. Pakai `validate_wiring()`.
2. **Event id buatan sendiri** — urutan same-tick bisa berubah. Pakai
   `Event::at`.
3. **State hanya di field privat** — observer/checkpoint tidak melihatnya.
   Mirror ke `State`.
4. **Menganggap `on_mount` langsung dieksekusi** — event yang dikembalikan baru
   diproses saat `run_until`/`run_ticks`.
5. **Lupa `shutdown()`** — `on_unmount` tidak dipanggil.
6. **Loop sendiri di dalam komponen** — komponen harus reaktif terhadap event;
   waktu hanya bergerak lewat engine.
