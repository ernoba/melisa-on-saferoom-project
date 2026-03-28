# Deployment Engine — Cara Kerja Internal

Dokumen ini menjelaskan cara kerja `mel_parser.rs` dan `deployer.rs` secara mendalam — untuk kontributor yang ingin memahami atau memodifikasi deployment pipeline.

---

## Gambaran Besar

```
File .mel (TOML)
       │
       ▼
  mel_parser.rs          → MelManifest (typed struct)
       │
       ▼
  deployer.rs            → 7 langkah deployment berurutan
       │
       ├── [STEP 1/7]  container.rs   → Provision / start container
       ├── [STEP 2/7]  dependency.rs  → Deteksi package manager
       ├── [STEP 3/7]  dependency.rs  → Install paket sistem
       ├── [STEP 4/7]  dependency.rs  → Install paket bahasa
       ├── [STEP 5/7]  deployer.rs    → Konfigurasi volumes
       ├── [STEP 6/7]  deployer.rs    → Injeksi env vars
       └── [STEP 7/7]  deployer.rs    → Lifecycle hooks + health check
```

---

## `mel_parser.rs` — Parsing TOML ke Struct

### Fungsi Utama: `load_mel_file`

```rust
pub async fn load_mel_file(path: &str) -> Result<MelManifest, Box<dyn Error>>
```

Urutan eksekusi:

1. Baca file dari disk secara async dengan `tokio::fs::read_to_string`
2. Parse string TOML menjadi `MelManifest` dengan `toml::from_str`
3. Kembalikan `Ok(manifest)` atau `Err` jika file tidak ada atau TOML tidak valid

Implementasi yang disederhanakan:

```rust
pub async fn load_mel_file(path: &str) -> Result<MelManifest, Box<dyn Error>> {
    let content = tokio::fs::read_to_string(path).await?;
    let manifest: MelManifest = toml::from_str(&content)?;
    Ok(manifest)
}
```

Semua kerja berat dilakukan oleh crate `toml` — MELISA mendapatkan validasi tipe secara gratis dari Rust's type system. Jika field yang dideklarasikan `String` di TOML-nya berisi integer, parsing langsung gagal dengan pesan error yang deskriptif.

### Serde dan TOML Deserialization

Setiap struct menggunakan derive macro `#[derive(Debug, Deserialize)]`. Field opsional menggunakan `Option<T>` dan diberi anotasi `#[serde(default)]` atau `#[serde(skip_serializing_if)]`:

```rust
#[derive(Debug, Deserialize)]
pub struct ContainerSection {
    pub distro:     String,
    pub name:       Option<String>,          // opsional → None jika tidak diisi
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,                     // default true
}

fn default_auto_start() -> bool { true }
```

Section opsional di level manifest-root menggunakan `#[serde(default)]`:

```rust
#[derive(Debug, Deserialize)]
pub struct MelManifest {
    pub project:      ProjectSection,
    pub container:    ContainerSection,
    #[serde(default)]
    pub env:          HashMap<String, String>,   // {} jika section tidak ada
    #[serde(default)]
    pub dependencies: DependencySection,
    // ...
    pub health:       Option<HealthSection>,     // None jika section tidak ada
}
```

Perbedaan `#[serde(default)]` vs `Option<T>`:
- `#[serde(default)]` → field ada tapi kosong (Vec kosong, HashMap kosong)
- `Option<T>` → section sepenuhnya tidak ada di file `.mel`

### Method `effective_name`

```rust
impl ContainerSection {
    pub fn effective_name(&self, project_name: &str) -> String {
        match &self.name {
            Some(n) => n.clone(),
            None => project_name
                .to_lowercase()
                .replace(' ', "-"),
        }
    }
}
```

Jika `container.name` diisi, digunakan apa adanya. Jika tidak, nama project dinormalisasi: lowercase + spasi menjadi `-`.

---

## `dependency.rs` — Pembangunan Perintah Instalasi

### `build_update_cmd`

Mengembalikan string perintah untuk memperbarui index package manager:

```rust
pub fn build_update_cmd(pkg_manager: &str) -> &'static str {
    match pkg_manager {
        "apt-get" | "apt" => "apt-get update -y",
        "dnf"             => "dnf makecache",
        "apk"             => "apk update",
        "pacman"          => "pacman -Sy --noconfirm",
        "zypper"          => "zypper --non-interactive refresh",
        _                 => "true",  // no-op untuk PM tidak dikenal
    }
}
```

### `build_system_install_cmd`

Menggabungkan semua package dari field yang sesuai dengan PM yang terdeteksi:

```rust
pub fn build_system_install_cmd(
    pkg_manager: &str,
    deps: &DependencySection,
) -> Option<String> {
    let packages: &[String] = match pkg_manager {
        "apt-get" | "apt" => &deps.apt,
        "dnf"             => &deps.dnf,
        "apk"             => &deps.apk,
        "pacman"          => &deps.pacman,
        _                 => return None,  // PM tidak dikenal → None
    };

    if packages.is_empty() {
        return None;  // tidak ada paket untuk PM ini
    }

    let pkg_list = packages.join(" ");
    let cmd = match pkg_manager {
        "apt-get" | "apt" => format!("apt-get install -y {}", pkg_list),
        "dnf"             => format!("dnf install -y {}", pkg_list),
        "apk"             => format!("apk add {}", pkg_list),
        "pacman"          => format!("pacman -S --noconfirm {}", pkg_list),
        _                 => return None,
    };
    Some(cmd)
}
```

### `has_lang_deps`

Fungsi boolean yang mengecek apakah ada setidaknya satu bahasa yang perlu diinstal:

```rust
pub fn has_lang_deps(deps: &DependencySection) -> bool {
    !deps.pip.is_empty()      ||
    !deps.npm.is_empty()      ||
    !deps.cargo.is_empty()    ||
    !deps.gem.is_empty()      ||
    !deps.composer.is_empty()
}
```

Digunakan oleh `deployer.rs` untuk menentukan apakah Step 4 perlu dijalankan.

### Deteksi Package Manager di Dalam Container

Step 2 deployment menjalankan serangkaian perintah `which` di dalam container untuk mendeteksi PM yang tersedia:

```rust
async fn detect_pkg_manager(container: &str) -> String {
    let candidates = [
        ("apt-get", "which apt-get"),
        ("dnf",     "which dnf"),
        ("apk",     "which apk"),
        ("pacman",  "which pacman"),
        ("zypper",  "which zypper"),
    ];

    for (name, cmd) in &candidates {
        let output = Command::new("sudo")
            .args(&["-n", "lxc-attach", "-n", container, "--", "sh", "-c", cmd])
            .output()
            .await;
        if let Ok(out) = output {
            if out.status.success() {
                return name.to_string();
            }
        }
    }
    "unknown".to_string()
}
```

---

## `deployer.rs` — 7 Langkah Deployment

### Entry Point: `run_deployment`

```rust
pub async fn run_deployment(
    manifest_path: &str,
    pb: &ProgressBar,
    audit: bool,
) {
    // 1. Parse manifest
    let manifest = match load_mel_file(manifest_path).await {
        Ok(m)  => m,
        Err(e) => { pb.println(format!("[ERROR] {}", e)); return; }
    };

    let container = manifest.container.effective_name(&manifest.project.name);

    pb.println(format!("[INFO] Deploying '{}' to container '{}'", manifest.project.name, container));

    // 2-7. Jalankan setiap step
    step_1_provision(&container, &manifest.container, pb, audit).await;
    let pkg_mgr = step_2_detect_pm(&container, pb).await;
    step_3_install_system_deps(&container, &pkg_mgr, &manifest.dependencies, pb, audit).await;
    step_4_install_lang_deps(&container, &manifest.dependencies, pb, audit).await;
    step_5_configure_volumes(&container, &manifest.volumes, pb).await;
    step_6_inject_env(&container, &manifest.env, pb, audit).await;
    step_7_lifecycle_and_health(&container, &manifest.lifecycle, &manifest.health, pb, audit).await;
}
```

### Step 1: Provision Container

Memeriksa apakah container dengan nama tersebut sudah ada:
- **Sudah ada + RUNNING:** langsung lanjut ke Step 2
- **Sudah ada + STOPPED:** jalankan `lxc-start` (jika `auto_start = true`)
- **Belum ada:** panggil `create_new_container` dari `container.rs`

### Step 2: Deteksi Package Manager

Memanggil `detect_pkg_manager()` via `lxc-attach`. Hasilnya digunakan di Step 3.

### Step 3: Install Paket Sistem

```
[STEP 3/7]  Installing system dependencies
[INFO]      apt-get update -y
[INFO]      apt-get install -y curl git nginx
```

1. Jalankan `build_update_cmd(pkg_mgr)` untuk refresh index
2. Jalankan `build_system_install_cmd(pkg_mgr, &deps)` untuk install

Jika `build_system_install_cmd` mengembalikan `None` (tidak ada paket untuk PM ini), step ini di-skip dengan pesan `[SKIP]`.

### Step 4: Install Paket Bahasa

```
[STEP 4/7]  Installing language dependencies
[INFO]      pip3 install flask gunicorn
[INFO]      npm install -g express
```

Hanya dijalankan jika `has_lang_deps()` mengembalikan `true`. Setiap bahasa diproses satu per satu dan hasilnya dilaporkan.

### Step 5: Konfigurasi Volumes

```
[STEP 5/7]  Configuring volumes
[INFO]      Mounting /home/alice/myapp → /app
[INFO]      Container restart required to apply new mounts
```

1. Baca file config LXC (`/var/lib/lxc/<n>/config`)
2. Cek apakah mount entry sudah ada (idempotent)
3. Jika belum ada: append konfigurasi mount dan restart container
4. Jika sudah ada: skip restart

Format yang ditambahkan:

```
lxc.mount.entry = /host/path /var/lib/lxc/CONTAINER/rootfs/container/path none bind,create=dir 0 0
```

### Step 6: Injeksi Environment Variables

```
[STEP 6/7]  Injecting environment variables
[INFO]      APP_PORT=8080
[INFO]      DB_HOST=localhost
```

Untuk setiap pasangan key-value di `manifest.env`, fungsi `build_env_inject_cmd` membangun perintah idempotent:

```rust
pub fn build_env_inject_cmd(key: &str, value: &str) -> String {
    format!(
        "sed -i '/^{}=/d' /etc/environment && echo '{}={}' >> /etc/environment",
        key, key, value
    )
}
```

Pola `sed -i '/^KEY=/d'` menghapus entry lama sebelum menambahkan yang baru, sehingga `--up` aman diulang tanpa duplikasi.

### Step 7: Lifecycle Hooks dan Health Check

```
[STEP 7/7]  Running lifecycle hooks (on_create)
[INFO]      mkdir -p /app/logs
[INFO]      chown -R www-data:www-data /app
[INFO]      Running health check: curl -sf http://localhost:8080/health
[SUCCESS]   Application is healthy!
```

#### Eksekusi Lifecycle Hooks

Setiap perintah di `on_create` dieksekusi secara berurutan via `lxc-attach`:

```rust
for cmd in &manifest.lifecycle.on_create {
    pb.println(format!("[INFO] Running: {}", cmd));
    let status = Command::new("sudo")
        .args(&["-n", "lxc-attach", "-n", &container, "--", "sh", "-c", cmd])
        .output()
        .await;
    // log success atau error, tapi SELALU lanjutkan ke hook berikutnya
}
```

Kegagalan satu hook tidak menghentikan deployment — filosofinya adalah "do best effort" dan laporkan hasil ke operator.

#### Retry Plan Health Check

```rust
pub struct HealthRetryPlan {
    pub command:       String,
    pub retries:       u32,
    pub interval_secs: u64,
}

pub fn build_health_check_retry_plan(h: &HealthSection) -> HealthRetryPlan {
    HealthRetryPlan {
        command:       h.command.clone(),
        retries:       h.retries.unwrap_or(3),
        interval_secs: h.interval.unwrap_or(5),
    }
}
```

Engine mengeksekusi health check command di dalam container dengan retry loop:

```rust
for attempt in 1..=plan.retries {
    let result = /* lxc-attach -- sh -c plan.command */;
    if result.success() {
        pb.println("[SUCCESS] Application is healthy!");
        return;
    }
    pb.println(format!("[INFO] Health check attempt {}/{} failed, retrying in {}s...",
        attempt, plan.retries, plan.interval_secs));
    sleep(Duration::from_secs(plan.interval_secs)).await;
}
pb.println("[WARNING] Health check failed after all retries. Verify application manually.");
```

---

## Flag `--audit`

Semua fungsi deployment menerima parameter `audit: bool`. Ketika `true`:

1. **Spinner disembunyikan** — progress bar tidak ditampilkan
2. **Raw output di-inherit** — stdout/stderr dari subprocess langsung mengalir ke terminal
3. **Debug messages ditampilkan** — pesan `[AUDIT]` tambahan muncul

Implementasinya menggunakan cabang `if audit { Stdio::inherit() } else { Stdio::piped() }`:

```rust
if audit {
    Command::new("sudo")
        .args(&["-n", "lxc-attach", ...])
        .stdout(Stdio::inherit())  // stream langsung ke terminal
        .stderr(Stdio::inherit())
        .status()
        .await
} else {
    Command::new("sudo")
        .args(&["-n", "lxc-attach", ...])
        .output()  // capture, jangan tampilkan kecuali ada error
        .await
}
```

---

## `--down`: Teardown Deployment

```
melisa@host:~> melisa --down ./myapp/program.mel
```

Teardown mengikuti urutan terbalik dari deployment:

1. Parse `.mel` untuk mendapatkan nama container
2. Jalankan `on_stop` lifecycle hooks
3. Hentikan container dengan `lxc-stop`

Teardown tidak menghapus container — hanya menghentikannya. Gunakan `--delete <n>` untuk menghapus container secara permanen.

---

## `--mel-info`: Preview Manifest

```
melisa@host:~> melisa --mel-info ./myapp/program.mel
```

Mem-parse file `.mel` dan menampilkan summary tanpa melakukan apa-apa:

```
━━━ Manifest Info: ./myapp/program.mel ━━━

PROJECT:
  name:        flask-api
  version:     2.1.0
  description: REST API dengan Flask dan PostgreSQL

CONTAINER:
  distro:      ubuntu/jammy/amd64
  name:        flask-api-prod
  auto_start:  true

DEPENDENCIES:
  system (apt): curl, git, nginx, postgresql
  pip:          flask, gunicorn, psycopg2-binary

VOLUMES:
  /home/alice/flask-api → /app

PORTS:
  expose: 5000, 5432

LIFECYCLE:
  on_create: 3 commands
  on_stop:   1 command

HEALTH CHECK:
  command:  curl -sf http://localhost:5000/api/health
  interval: 5s  retries: 3
```

Berguna untuk memverifikasi manifest sebelum menjalankan `--up`.