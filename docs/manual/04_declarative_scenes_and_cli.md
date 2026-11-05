# Bab 04: Format Deklaratif `.twscene` & Panduan CLI

Tech World Engine menyediakan format deklaratif **`.twscene`** (berbasis JSON) yang memungkinkan Anda mendefinisikan seluruh dunia simulasi—komponen, parameter, pengkabelan port, resource, dan stimulus awal—secara deklaratif tanpa perlu mengompilasi ulang kode Rust.

---

## 1. Spesifikasi Format File `.twscene`

File skema dunia `.twscene` adalah dokumen JSON yang terstruktur. Berikut adalah contoh lengkapnya:

```json
{
  "name": "network-pipeline-demo",
  "ticks_per_second": 1000,
  "resources": [
    {
      "id": 100,
      "name": "CoreRouterBuffer",
      "capacity": 10,
      "service_time": 2
    }
  ],
  "components": [
    {
      "id": 1,
      "kind": "Counter",
      "params": { "name": "PacketSource", "limit": 10, "interval": 5 }
    },
    {
      "id": 2,
      "kind": "Gate",
      "params": { "gate": "AND" }
    }
  ],
  "connections": [
    {
      "from": { "component": 1, "port": "out" },
      "to":   { "component": 2, "port": "in_a" }
    }
  ],
  "stimuli": [
    { "at": 0, "component": 1, "tick": true },
    { "at": 10, "component": 2, "port": "in_b", "value": 1 }
  ]
}
```

### Rincian Bidang (*Fields*):

| Bidang | Tipe | Keterangan |
| --- | --- | --- |
| `name` | String | Nama identifikasi skenario dunia. |
| `ticks_per_second` | Integer | Rasio konversi tick mesin terhadap detik dunia (dipakai oleh visualizer/Godot). |
| `resources[]` | Array | Pool kapasitas bersama: `id`, `name`, `capacity`, dan `service_time` (latensi otomatis). |
| `components[]` | Array | Daftar komponen yang diinstansiasi: `id`, `kind` (kunci di Registry), dan `params` (JSON bebas untuk dikonsumsi factory). |
| `connections[]` | Array | Pengkabelan antar-komponen menggunakan **nama port string** (`from: {component, port}`, `to: {component, port}`). |
| `stimuli[]` | Array | Sinyal awal yang dijadwalkan saat t=0 atau tick tertentu: `tick: true` (event clock generik) atau `port` + `value` (sinyal data). |

> [!TIP]
> **Mengapa Port Dihubungkan Berdasarkan Nama?**  
> Di level file konfigurasi, port dirujuk dengan nama string seperti `"in_a"`, `"out_y"`, bukan integer ID `PortId(101)`. Hal ini membuat file `.twscene` tetap stabil dan tidak rusak saat refactoring kode internal komponen.

---

## 2. Component Registry (`tw_scene::Registry`)

Registry bertindak sebagai jembatan yang menghubungkan nama string `kind` di file `.twscene` dengan kode instansiasi komponen di Rust.

### Komponen Bawaan (*Builtin Registry*)
Saat memanggil `Registry::builtin()`, jenis komponen berikut langsung tersedia:
* **`Counter`**: Parameter `limit`, `interval`, `name`.
* **`Gate`**: Parameter `gate` (`"XOR"`, `"AND"`, `"OR"`).
* **`Probe`**: Pengumpul sinyal logika.
* **`Worker`**: Parameter `resource`, `work_duration`, `name`.
* **`DiskJob`**: Parameter `resource`, `name`.

### Mendaftarkan Komponen Kustom Pengguna
Anda dapat dengan mudah mendaftarkan factory komponen buatan Anda sendiri:

```rust
use tw_scene::Registry;

let mut reg = Registry::builtin();

// Daftarkan komponen PacketFilter buatan kita dari Bab 03
reg.register("PacketFilter", |spec| {
    let max_size = spec.params.get("max_size").and_then(|v| v.as_i64()).unwrap_or(1500);
    let name = spec.params.get("name").and_then(|v| v.as_str()).unwrap_or("Filter");
    Ok(Box::new(PacketFilter::new(spec.id, name, max_size)))
});
```

---

## 3. Client Façade: `Session`

[`Session`](file:///home/rkcode/Projects/tech-world-engine/crates/tw-scene/src/session.rs) adalah satu-satunya antarmuka yang perlu diketahui oleh klien luar (baik CLI maupun GDExtension Godot).

```rust
use tw_scene::Session;

// 1. Muat skenario dunia dari file
let mut session = Session::load("scenes/counter.twscene")?;

// 2. Majukan waktu simulasi sebanyak 50 ticks
let report = session.step(50);
println!("Tick saat ini: {}, Event yang dieksekusi: {}", report.tick, report.events_executed);

// 3. Ambil perubahan variabel sejak frame sebelumnya (State Diffing)
let changes = session.take_state_changes();
for change in changes {
    println!("Komponen #{} variabel '{}' berubah menjadi {:?}", 
        change.component.0, change.key, change.new);
}

// 4. Berikan stimulus interaktif dari luar
session.inject_port_signal(ComponentId(1), "in_a", StateValue::from(1))?;
```

---

## 4. Panduan Penggunaan CLI (`tw-cli`)

CLI engine (`crates/tw-cli`) menyediakan antarmuka baris perintah yang serbaguna untuk menjalankan dan menginspeksi file `.twscene`.

### A. Melihat Daftar Jenis Komponen yang Terdaftar
```bash
cargo run -p tw-cli -- list-kinds
```
*Output:*
```text
Available Component Kinds in Registry:
  - Counter
  - DiskJob
  - Gate
  - Probe
  - Worker
```

### B. Menjalankan File Skenario (`run`)
Jalankan simulasi dari file `.twscene` untuk durasi ticks tertentu:
```bash
cargo run -p tw-cli -- run scenes/counter.twscene --ticks 50
```
*Output:*
```text
Running scene 'counter-demo' for 50 ticks...
Finished at tick 50, executed 10 events.

Final Component States:
  #1: 'Counter1'
    count = 10
```

### C. Menginspeksi Komponen & Port (`inspect`)
Melihat seluruh komponen, arah port, dan state variabelnya tanpa harus menjalankan simulasi sampai selesai:
```bash
cargo run -p tw-cli -- inspect scenes/logic_gates.twscene
```
*Output:*
```text
Scene: 'logic-gates-demo' (at tick 0)
Components (2):

  Component #1: 'XOR'
    Ports: in_a:Input, in_b:Input, out_y:Output
    State Variables:

  Component #2: 'AND'
    Ports: in_a:Input, in_b:Input, out_y:Output
    State Variables:
```

### D. Melacak Kausalitas / Debugging Event (`trace`)
Menjawab pertanyaan: *"Mengapa komponen #2 memiliki state ini?"*
```bash
cargo run -p tw-cli -- trace scenes/logic_gates.twscene --component 2
```
*Output:*
```text
Component #2 has 2 trace events. Most recent causal chain:
  [1] t=0 Trace #4 (Event #4): Event #4 [PortSignal { port_id: PortId(112), value: Int(1) }] executed on Component 'AND' (#2)
```

CLI akan mencetak rantai kausalitas lengkap terurut dari akar pemicu awal hingga efek akhir pada komponen tersebut!
