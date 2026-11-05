# Bab 03: Panduan Pembuatan Komponen Mandiri

Sebagai engine generik, Tech World Engine dirancang agar Anda dapat membuat komponen domain apa pun—mulai dari sakelar sederhana, kontroler lift, router jaringan, hingga stasiun kerja perakitan pabrik—hanya dengan mengimplementasikan trait [`Component`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-core/src/component/mod.rs).

---

## 1. Model Mental Komponen

Sebuah komponen adalah **mesin transisi state reaktif**:

```text
               Event Masuk (Sinyal Port / Tick / Resource)
                                   │
                                   ▼
                   ┌───────────────────────────────┐
                   │    Component::handle_event    │
                   └───────────────┬───────────────┘
                                   │
                ┌──────────────────┴──────────────────┐
                ▼                                     ▼
        Perbarui State                          Kembalikan Event Baru
  (Misal: count += 1, status)               (Misal: sinyal ke port output)
```

> [!NOTE]
> Komponen **tidak memiliki thread sendiri** dan **tidak boleh memajukan waktu sendiri**. Waktu simulasi hanya bertambah ketika engine mengeksekusi antrean event.

---

## 2. Struktur Trait `Component`

Trait `Component` didefinisikan di `tw_core`:

```rust
use tw_core::{
    ComponentId, Event, Port, SimulationContext, State,
};

pub trait Component: std::fmt::Debug {
    // 1. Identitas & Port
    fn id(&self) -> ComponentId;
    fn name(&self) -> &str;
    fn ports(&self) -> &[Port];

    // 2. State Publik
    fn state(&self) -> &State;

    // 3. Logika Transisi
    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event>;

    // 4. Lifecycle (Opsional)
    fn on_mount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> { Vec::new() }
    fn on_unmount(&mut self, _ctx: &mut SimulationContext) -> Vec<Event> { Vec::new() }
    fn restore_state(&mut self, _state: &State) -> bool { false }
}
```

### Rincian Method:
1. **`id()` & `name()`**: Mengembalikan ID unik komponen dan nama representasi untuk logging/visualisasi.
2. **`ports()`**: Mengembalikan daftar port yang dapat dihubungkan ke komponen lain.
3. **`state()`**: Mengembalikan referensi ke `State` publik yang diamati oleh observer, checkpoint, dan visualizer Godot/CLI.
4. **`handle_event()`**: Inti perilaku komponen. Menerima event saat ini dan konteks simulasi, lalu mengembalikan daftar event baru yang harus dijadwalkan engine.
5. **`on_mount()`**: Dipanggil sekali saat komponen didaftarkan ke `Simulation`. Berguna untuk menjadwalkan event inisialisasi awal.
6. **`on_unmount()`**: Dipanggil saat simulasi ditutup (`shutdown`).
7. **`restore_state()`**: Dipanggil saat engine memulihkan checkpoint (`sim.restore()`). Salin snapshot kembali ke field internal komponen dan kembalikan `true`.

---

## 3. Tutorial Langkah-Demi-Langkah: Membuat Komponen `PacketFilter`

Mari kita buat sebuah komponen pemfilter paket data di domain jaringan:
* Memiliki port input `"ingress"` dan port output `"egress"`.
* Menerima paket berupa angka integer ukuran paket (`bytes`).
* Jika `bytes > max_size`, paket dibuang (*dropped*) dan dicatat di state.
* Jika `bytes <= max_size`, paket diteruskan ke port `"egress"` dengan delay latensi 2 ticks.

### Langkah 1: Deklarasi Struct

```rust
use tw_core::{
    Component, ComponentId, Event, EventPayload, Port, PortDirection, PortId,
    SimulationContext, SimulationTime, State, StateValue,
};

const PORT_INGRESS: PortId = PortId(1);
const PORT_EGRESS: PortId = PortId(2);

#[derive(Debug)]
pub struct PacketFilter {
    id: ComponentId,
    name: String,
    max_size: i64,
    ports: Vec<Port>,
    state: State,
    // Field lokal
    forwarded_count: u64,
    dropped_count: u64,
}
```

### Langkah 2: Konstruktor & Inisialisasi Port

```rust
impl PacketFilter {
    pub fn new(id: ComponentId, name: impl Into<String>, max_size: i64) -> Self {
        let ports = vec![
            Port::new(PORT_INGRESS, "ingress", PortDirection::Input),
            Port::new(PORT_EGRESS, "egress", PortDirection::Output),
        ];

        let mut state = State::new();
        state.set("forwarded", 0);
        state.set("dropped", 0);

        Self {
            id,
            name: name.into(),
            max_size,
            ports,
            state,
            forwarded_count: 0,
            dropped_count: 0,
        }
    }
}
```

### Langkah 3: Implementasi Perilaku (`handle_event`)

```rust
impl Component for PacketFilter {
    fn id(&self) -> ComponentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn ports(&self) -> &[Port] { &self.ports }
    fn state(&self) -> &State { &self.state }

    fn handle_event(&mut self, event: &Event, ctx: &mut SimulationContext) -> Vec<Event> {
        let mut next_events = Vec::new();

        // Periksa apakah event adalah sinyal pada port input
        if let EventPayload::PortSignal { port_id, value } = &event.payload {
            if *port_id == PORT_INGRESS {
                let packet_size = value.as_int().unwrap_or(0);

                if packet_size > self.max_size {
                    // Paket terlalu besar -> buang
                    self.dropped_count += 1;
                    self.state.set("dropped", self.dropped_count);
                } else {
                    // Paket lolos -> jadwalkan pengiriman ke port egress (delay 2 tick)
                    self.forwarded_count += 1;
                    self.state.set("forwarded", self.forwarded_count);

                    let send_time = SimulationTime::new(ctx.time().ticks() + 2);
                    next_events.push(Event::at(
                        send_time,
                        self.id,  // source (engine akan mencocokkan ke koneksi port)
                        self.id,  // target
                        EventPayload::PortSignal {
                            port_id: PORT_EGRESS,
                            value: value.clone(),
                        },
                    ));
                }
            }
        }

        next_events
    }

    // Mendukung pemulihan snapshot checkpoint
    fn restore_state(&mut self, state: &State) -> bool {
        if let (Some(f), Some(d)) = (state.get_int("forwarded"), state.get_int("dropped")) {
            self.forwarded_count = f as u64;
            self.dropped_count = d as u64;
            self.state = state.clone();
            true
        } else {
            false
        }
    }
}
```

---

## 4. Merangkai & Menjalankan di `tw-sim`

Setelah komponen selesai dibuat, Anda bisa merangkainya dengan komponen lain dan memvalidasinya secara otomatis:

```rust
use tw_sim::Simulation;
use tw_core::ComponentId;

let mut sim = Simulation::new();

let filter = PacketFilter::new(ComponentId(1), "CoreFilter", 1500);
sim.register_component(Box::new(filter));

// Hubungkan output filter ke input komponen penerima menggunakan nama port:
sim.connect(ComponentId(1), "egress", ComponentId(2), "ingress")
    .expect("koneksi valid");

// Validasi seluruh koneksi sirkuit
sim.validate_wiring().expect("seluruh wiring sirkuit valid");
```

---

## 5. Bekerja dengan Sumber Daya Terbatas (*Generic Resources*)

Jika komponen Anda memerlukan kapasitas bersama (misalnya satu inti CPU, slot memori, atau jalur disk), gunakan primitive **Resource**:

1. Daftarkan resource di simulasi:
   ```rust
   use tw_core::{Amount, Resource, ResourceId};
   sim.register_resource(Resource::new(ResourceId(10), "CPU", Amount::Discrete(1)).with_service_time(5));
   ```
2. Minta alokasi dari komponen:
   ```rust
   let req = Event::at(
       ctx.time(),
       self.id,
       self.id,
       EventPayload::ResourceRequested {
           resource_id: ResourceId(10),
           amount: Amount::Discrete(1),
           requester: self.id,
       },
   );
   ```
3. Komponen Anda hanya perlu menangani respon `EventPayload::ResourceGranted` atau `ResourceDenied` pada `handle_event`. Setelah `service_time` (5 ticks) terlampaui, engine akan **otomatis melepaskan kapasitas** dan menjadwalkan `ResourceReleased`.

---

## 6. Checklist & Pitfall Umum

Sebelum mendistribusikan komponen Anda, pastikan:

- [ ] **Gunakan `Event::at`**: Jangan membuat `EventId` sendiri; engine yang akan mengisi ID otoritatif.
- [ ] **Waktu Kausal**: Jangan menjadwalkan event di masa lalu ($t < \text{ctx.time()}$).
- [ ] **Sinkronisasi State**: Selalu perbarui `self.state` setiap kali field internal berubah agar dapat dilihat oleh inspector dan observer.
- [ ] **Gunakan `validate_wiring()`**: Selalu jalankan validasi setelah menyambungkan kabel port untuk mencegah salah ketik port yang membuat sinyal diam-diam hilang.
- [ ] **Uji Determinis**: Jalankan simulasi dua kali dengan input yang sama; verifikasi urutan event dan state akhir 100% identik.
