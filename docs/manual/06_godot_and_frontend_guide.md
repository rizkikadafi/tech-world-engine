# Bab 06: Panduan Integrasi Godot & Client Eksternal

Bab ini menjelaskan bagaimana menghubungkan Tech World Engine ke **Godot Engine 4.x** (atau antarmuka grafis/web lainnya) menggunakan arsitektur **GDExtension Tipis (*Thin GDExtension Adapter*)**.

---

## 1. Posisi Frontend dalam Visi Engine

Prinsip fundamental dari proyek ini adalah:
> **Godot bertindak sebagai klien visualizer dan inspector, BUKAN tempat logika simulasi hidup.**

Seluruh `Time`, `State`, `Event Queue`, `Solver`, `Circuit`, dan `Component` tetap sepenuhnya milik engine Rust murni. Godot hanya melakukan:
* `visualize()`: Menggambar komponen, kabel, nilai sinyal, dan animasi.
* `inspect()`: Membaca port dan isi variabel state komponen.
* `stimulate()`: Mengirim sinyal klik tombol atau sakelar pengguna ke port komponen.
* `control()`: Mem-pause, memajukan step-by-step, atau me-restore checkpoint.
* `trace()`: Menampilkan diagram grafik kausalitas saat pengguna mengklik komponen (*"Kenapa sakelar ini mati?"*).

```text
┌──────────────────────────────────────────────┐
│             Godot 4 Application              │
│  (UI Inspector, 2D/3D Node, Animasi, Input)  │
└──────────────────────┬───────────────────────┘
                       │ Panggilan GDScript / Signals
                       ▼
┌──────────────────────────────────────────────┐
│          tw-godot (GDExtension Tipis)        │ <- CDYLIB ADAPTER
│  - Menerjemahkan Variant <-> StateValue      │
│  - Akumulator delta detik -> ticks           │
│  - Memancarkan Godot Signals dari diff       │
└──────────────────────┬───────────────────────┘
                       │ Panggilan Rust Murni
                       ▼
┌──────────────────────────────────────────────┐
│              tw-scene (Session)              │ <- CLIENT FAÇADE
└──────────────────────┬───────────────────────┘
                       ▼
┌──────────────────────────────────────────────┐
│              tw-sim & tw-core                │ <- SIMULATION KERNEL
└──────────────────────────────────────────────┘
```

---

## 2. Apa yang Dimaksud "GDExtension Tipis"?

### ❌ Pendekatan Tebal (Yang Dihindari)
Memasukkan antrean event, state simulasi, atau persamaan rangkaian langsung ke dalam node-node Godot. Pendekatan ini membuat simulasi tidak bisa diuji tanpa membuka game editor, rawan bug floating-point delta, dan membebani performa engine game.

### ✅ Pendekatan Tipis (Yang Diterapkan)
Crate GDExtension (`tw-godot`) **hanya berupa lem tipis (*thin glue code*)** yang membungkus [`Session`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-scene/src/session.rs).
1. `tw-godot` tidak memiliki logika simulasi internal.
2. `tw-core`, `tw-sim`, dan `tw-scene` tidak pernah mengimpor library Godot sama sekali.
3. Anda dapat mencopot Godot kapan saja; simulasi tetap berjalan utuh 100% dari CLI.

---

## 3. Sinkronisasi Waktu: Delta Detik ke Integer Ticks

Godot berjalan pada frame rate dinamis dengan waktu kontinu floating-point (`delta` detik pada `_process(delta)`). Sebaliknya, Tech World Engine berjalan pada **fixed-point integer ticks** deterministik (`u64 ticks`).

Jembatan sinkronisasi waktu dilakukan oleh **Tick Accumulator**:

$$
\text{accumulated\_time} \mathrel{+}= \text{delta} \times \text{ticks\_per\_second}
$$

```text
Godot Frame 1: delta = 0.016s -> ticks_to_step = 16 ticks -> session.step(16)
Godot Frame 2: delta = 0.017s -> ticks_to_step = 17 ticks -> session.step(17)
```

Jika simulasi di-pause oleh user, `session.step()` tidak dipanggil, sehingga waktu simulasi berhenti tanpa memengaruhi frame rate render Godot.

---

## 4. Visualisasi Reaktif Efisien via `take_state_changes()`

Memeriksa dan membaca seluruh state komponen di setiap frame 60 FPS (*polling*) akan sangat lambat jika ada ribuan komponen di dalam scene.

Engine menyediakan metode **diffing state otomatis**:
```rust
let changes = session.take_state_changes();
```
Metode ini hanya mengembalikan properti komponen yang **benar-benar berubah nilai** sejak panggilan terakhir. GDExtension cukup mengubahnya menjadi Godot Signal:

```rust
for change in session.take_state_changes() {
    self.base_mut().emit_signal(
        "state_changed".into(),
        &[
            change.component.0.to_variant(),
            change.key.to_variant(),
            change.new.to_string().to_variant(),
        ],
    );
}
```

---

## 5. Cetak Biru Implementasi (`tw-godot`)

Berikut adalah kerangka implementasi node `SimulationHost` yang menghubungkan Godot ke `Session`:

### File: `crates/tw-godot/src/lib.rs`

```rust
use godot::prelude::*;
use tw_core::{ComponentId, StateValue};
use tw_scene::Session;

struct TechWorldExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TechWorldExtension {}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SimulationHost {
    base: Base<Node>,
    session: Option<Session>,
    accumulated_time: f64,
    pub is_paused: bool,
}

#[godot_api]
impl INode for SimulationHost {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            session: None,
            accumulated_time: 0.0,
            is_paused: false,
        }
    }

    fn process(&mut self, delta: f64) {
        if self.is_paused {
            return;
        }

        if let Some(session) = &mut self.session {
            let tps = session.world().ticks_per_second as f64;
            self.accumulated_time += delta * tps;

            let ticks_to_step = self.accumulated_time.floor() as u64;
            if ticks_to_step > 0 {
                self.accumulated_time -= ticks_to_step as f64;
                session.step(ticks_to_step);

                // Pancarkan sinyal hanya untuk state yang berubah
                let changes = session.take_state_changes();
                for change in changes {
                    let comp_id = change.component.0 as i64;
                    let key = GString::from(change.key);
                    let val = GString::from(change.new.to_string());

                    self.base_mut().emit_signal(
                        "state_changed".into(),
                        &[comp_id.to_variant(), key.to_variant(), val.to_variant()],
                    );
                }
            }
        }
    }
}

#[godot_api]
impl SimulationHost {
    #[signal]
    fn state_changed(component_id: i64, key: GString, new_value: GString);

    #[func]
    pub fn load_scene(&mut self, file_path: GString) -> bool {
        match Session::load(file_path.to_string()) {
            Ok(s) => {
                self.session = Some(s);
                true
            }
            Err(e) => {
                godot_error!("Failed to load scene: {e}");
                false
            }
        }
    }

    #[func]
    pub fn inject_port_signal(&mut self, comp_id: i64, port_name: GString, value_int: i64) -> bool {
        if let Some(session) = &mut self.session {
            session
                .inject_port_signal(
                    ComponentId(comp_id as u64),
                    &port_name.to_string(),
                    StateValue::from(value_int),
                )
                .is_ok()
        } else {
            false
        }
    }

    #[func]
    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    #[func]
    pub fn resume(&mut self) {
        self.is_paused = false;
    }
}
```

---

## 6. Contoh Penggunaan di GDScript

Di sisi Godot, skrip GDScript menjadi sangat sederhana dan deklaratif:

```gdscript
extends Node2D

@onready var sim_host: SimulationHost = $SimulationHost
@onready var counter_label: Label = $UI/CounterLabel
@onready var led_light: ColorRect = $UI/LedIndicator

func _ready():
    # 1. Sambungkan sinyal perubahan state dari engine Rust
    sim_host.state_changed.connect(_on_simulation_state_changed)
    
    # 2. Muat file skenario dunia .twscene
    sim_host.load_scene("res://scenes/counter.twscene")

func _on_simulation_state_changed(component_id: int, key: String, new_value: String):
    # Komponen #1 (Counter) variabel 'count'
    if component_id == 1 and key == "count":
        counter_label.text = "Count: " + new_value

    # Komponen #2 (Gerbang Logika) output 'y'
    elif component_id == 2 and key == "y":
        led_light.color = Color.GREEN if new_value == "1" else Color.DARK_GRAY

func _on_button_pressed():
    # Berikan stimulus interaktif dari input pengguna
    sim_host.inject_port_signal(2, "in_a", 1)
```

Dengan arsitektur ini, antarmuka visual Godot dapat dikembangkan dengan leluasa oleh artis atau desainer UI, sementara performa komputasi dan ketepatan kausalitas tetap terjamin 100% oleh engine Rust murni di belakangnya.
