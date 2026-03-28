# Format File `.mel` — Spesifikasi Lengkap

File `.mel` adalah manifest deklaratif berbasis TOML yang mendefinisikan bagaimana sebuah project di-deploy ke dalam LXC container di server MELISA. File ini di-parse oleh `src/deployment/mel_parser.rs` dan dieksekusi oleh `src/deployment/deployer.rs`.

---

## Ikhtisar

Setiap file `.mel` menjawab satu pertanyaan inti: *"Container seperti apa yang dibutuhkan project ini, dan bagaimana cara menyiapkannya dari nol?"*

```
melisa@host:~> melisa --up ./myapp/program.mel   # deploy
melisa@host:~> melisa --down ./myapp/program.mel # teardown
melisa@host:~> melisa --mel-info ./myapp/program.mel  # preview tanpa eksekusi
```

File `.mel` dibaca sekali, divalidasi strukturnya, lalu Engine menjalankan 7 langkah deployment secara berurutan.

---

## Struktur Section

File `.mel` terdiri dari section-section berikut. Section `[project]` dan `[container]` **wajib**; semua section lainnya bersifat opsional.

```
[project]      ← WAJIB — identitas project
[container]    ← WAJIB — target container dan distribusi
[env]          ← opsional — variabel lingkungan
[dependencies] ← opsional — paket sistem dan bahasa pemrograman
[volumes]      ← opsional — bind mount antara host dan container
[ports]        ← opsional — port yang akan di-expose (informational)
[lifecycle]    ← opsional — perintah yang dijalankan di lifecycle events
[services]     ← opsional — definisi service yang berjalan dalam container
[health]       ← opsional — health check setelah deployment
```

---

## Section `[project]`

**Status:** Wajib

Mendefinisikan identitas project. Field `name` adalah satu-satunya yang wajib diisi.

```toml
[project]
name        = "my-web-app"
version     = "1.0.0"
description = "Backend API untuk platform e-commerce"
author      = "Alice <alice@example.com>"
```

| Field | Tipe | Status | Keterangan |
|-------|------|--------|------------|
| `name` | string | **Wajib** | Nama project. Digunakan sebagai nama container default jika `[container].name` tidak diisi. Spasi dikonversi ke `-`, semua huruf dijadikan lowercase. Contoh: `"My App"` → container bernama `my-app`. |
| `version` | string | Opsional | Versi semantik project. Tidak digunakan oleh Engine, hanya informatif. |
| `description` | string | Opsional | Deskripsi singkat project. Ditampilkan saat `--mel-info`. |
| `author` | string | Opsional | Nama dan email pengelola. Ditampilkan saat `--mel-info`. |

### Aturan Penamaan (`name`)

Nama project mengalami normalisasi otomatis saat digunakan sebagai nama container:

```
"My Cool App"   → "my-cool-app"    (spasi → strip, lowercase)
"backend_api"   → "backend_api"    (underscore dibiarkan)
"WebService 2"  → "webservice-2"   (spasi → strip, lowercase)
```

Normalisasi ini dilakukan oleh method `ContainerSection::effective_name()` di `mel_parser.rs`.

---

## Section `[container]`

**Status:** Wajib

Menentukan distribusi Linux yang akan digunakan dan konfigurasi container.

```toml
[container]
distro     = "ubuntu/jammy/amd64"
name       = "myapp-prod"
auto_start = true
```

| Field | Tipe | Status | Default | Keterangan |
|-------|------|--------|---------|------------|
| `distro` | string | **Wajib** | — | Kode distribusi dalam format `name/release/arch`. Lihat bagian [Format `distro`](#format-field-distro) di bawah. |
| `name` | string | Opsional | Slugified `project.name` | Override nama container. Jika diisi, nama ini dipakai apa adanya tanpa normalisasi tambahan. |
| `auto_start` | boolean | Opsional | `true` | Jika `true`, Engine akan menjalankan container secara otomatis jika belum dalam status RUNNING saat `--up` dijalankan. |

### Format Field `distro`

Format: `<nama_distro>/<release>/<arsitektur>`

```toml
# Ubuntu
distro = "ubuntu/jammy/amd64"    # Ubuntu 22.04 LTS, 64-bit
distro = "ubuntu/focal/amd64"    # Ubuntu 20.04 LTS, 64-bit
distro = "ubuntu/noble/amd64"    # Ubuntu 24.04 LTS, 64-bit

# Debian
distro = "debian/bookworm/amd64" # Debian 12, 64-bit
distro = "debian/bullseye/amd64" # Debian 11, 64-bit

# Alpine Linux
distro = "alpine/3.18/amd64"
distro = "alpine/3.19/amd64"

# Fedora
distro = "fedora/39/amd64"

# Arch Linux
distro = "archlinux/current/amd64"
```

Untuk melihat daftar lengkap distribusi yang tersedia di server:

```
melisa@host:~> melisa --search ubuntu
melisa@host:~> melisa --search alpine
melisa@host:~> melisa --search debian
```

> **Catatan:** Nilai `distro` di `.mel` menggunakan format `name/release/arch` dengan separator `/`, berbeda dari shortcode pencarian (contoh `ubu-jammy-x64`) yang dipakai di `--search` dan `--create`. Engine membaca field `name`, `release`, dan `arch` secara terpisah dari string ini.

---

## Section `[env]`

**Status:** Opsional

Mendefinisikan variabel lingkungan yang akan diinjeksikan ke dalam container. Setiap pasangan key-value ditulis sebagai field TOML biasa di bawah section `[env]`.

```toml
[env]
APP_PORT      = "8080"
APP_ENV       = "production"
DB_HOST       = "localhost"
DB_PORT       = "5432"
DB_NAME       = "myapp_db"
DB_USER       = "appuser"
DB_PASS       = "secretpassword"
REDIS_URL     = "redis://localhost:6379"
JWT_SECRET    = "change-me-in-production"
LOG_LEVEL     = "info"
```

### Mekanisme Injeksi

Variabel diinjeksikan ke `/etc/environment` di dalam container menggunakan perintah `sed` yang idempotent — jika key sudah ada, nilainya diperbarui; jika belum ada, ditambahkan. Ini memastikan `--up` aman dijalankan berulang kali.

Pola injeksi yang digunakan (dari `deployer.rs`):

```bash
sed -i '/^KEY=/d' /etc/environment && echo 'KEY=VALUE' >> /etc/environment
```

Variabel tersedia untuk semua proses yang dijalankan di dalam container setelah container di-restart atau setelah session baru dibuka.

> **Peringatan keamanan:** Jangan simpan secret production (API key, password) langsung di file `.mel` jika file tersebut akan masuk ke version control. Gunakan file `.env` terpisah yang di-`.gitignore`, lalu sync dengan `melisa sync` (client akan menggunakan `rsync` untuk file `.env`).

---

## Section `[dependencies]`

**Status:** Opsional

Mendefinisikan paket yang akan diinstal di dalam container. Section ini dibagi menjadi dua kelompok: **paket sistem** (diinstal oleh package manager distro) dan **paket bahasa pemrograman**.

```toml
[dependencies]
# Paket sistem — diinstal sesuai package manager yang terdeteksi
apt     = ["curl", "git", "nginx", "build-essential"]
dnf     = []
apk     = []
pacman  = []

# Paket bahasa pemrograman — diinstal setelah paket sistem
pip      = ["flask", "gunicorn", "sqlalchemy", "redis"]
npm      = ["express", "dotenv", "axios"]
cargo    = ["ripgrep"]
gem      = ["rails", "puma"]
composer = ["laravel/framework"]
```

### Paket Sistem

Engine mendeteksi package manager yang tersedia di dalam container secara otomatis (lihat [Step 2: Deteksi Package Manager](../server-cli/deployment-engine.md)). Hanya list yang sesuai dengan package manager yang terdeteksi yang akan dieksekusi:

| Field | Package Manager | Perintah yang dihasilkan |
|-------|----------------|--------------------------|
| `apt` | `apt-get` | `apt-get install -y <pkg1> <pkg2> ...` |
| `dnf` | `dnf` | `dnf install -y <pkg1> <pkg2> ...` |
| `apk` | `apk` | `apk add <pkg1> <pkg2> ...` |
| `pacman` | `pacman` | `pacman -S --noconfirm <pkg1> <pkg2> ...` |

Jika container menggunakan Alpine tetapi hanya `apt` yang diisi, tidak ada paket sistem yang diinstal. Isi field yang sesuai dengan distribusi target, atau isi beberapa field jika manifest harus portabel lintas distro.

### Paket Bahasa Pemrograman

Diinstal setelah paket sistem selesai (Step 4 deployment). Engine menggunakan `lxc-attach` untuk menjalankan perintah instalasi di dalam container:

| Field | Perintah yang dijalankan di dalam container |
|-------|---------------------------------------------|
| `pip` | `pip3 install <pkg1> <pkg2> ...` |
| `npm` | `npm install -g <pkg1> <pkg2> ...` |
| `cargo` | `cargo install <pkg1> <pkg2> ...` |
| `gem` | `gem install <pkg1> <pkg2> ...` |
| `composer` | `composer require <pkg1> <pkg2> ...` |

> Engine hanya menjalankan instalasi bahasa jika setidaknya satu dari field `pip`, `npm`, `cargo`, `gem`, atau `composer` tidak kosong. Ini diperiksa oleh fungsi `has_lang_deps()` di `dependency.rs`.

---

## Section `[volumes]`

**Status:** Opsional

Mendefinisikan bind mount antara direktori di host dan path di dalam container.

```toml
[volumes]
mounts = [
    "/home/alice/myapp:/app",
    "/var/shared/data:/data",
    "/etc/ssl/certs:/etc/ssl/certs",
]
```

### Format Mount

Setiap entry mengikuti format `host_path:container_path`:

```
"/host/absolute/path:/container/absolute/path"
```

Kedua path harus menggunakan path absolut. Path relatif tidak didukung.

### Perilaku saat Deployment

Pada Step 5 deployment, Engine mengonfigurasi mount dengan cara:

1. Menulis konfigurasi mount ke file konfigurasi LXC container (`/var/lib/lxc/<name>/config`)
2. **Me-restart container** jika mount baru ditambahkan — ini diperlukan karena LXC membaca konfigurasi mount saat container start
3. Jika konfigurasi yang sama sudah ada (deployment ulang), langkah restart dilewati

Konfigurasi yang ditulis ke file LXC:

```
lxc.mount.entry = /host/path /container/path none bind,create=dir 0 0
```

---

## Section `[ports]`

**Status:** Opsional

Mendefinisikan port yang digunakan oleh aplikasi di dalam container. Section ini bersifat **informational** — MELISA tidak melakukan port forwarding otomatis. Gunakan `melisa tunnel` untuk mengakses port dari workstation lokal.

```toml
[ports]
expose = ["8080", "5432", "6379"]
```

Port yang terdaftar di sini ditampilkan saat `melisa --mel-info` dan pada summary akhir deployment sebagai panduan untuk perintah tunnel yang perlu dijalankan.

**Contoh output `--mel-info`:**

```
PORTS:
  expose: 8080, 5432, 6379

Tip: Gunakan `melisa tunnel myapp 8080` untuk mengakses port dari workstation lokal.
```

---

## Section `[lifecycle]`

**Status:** Opsional

Mendefinisikan perintah shell yang dieksekusi di dalam container pada momen-momen tertentu dalam siklus hidup deployment.

```toml
[lifecycle]
on_create = [
    "mkdir -p /app/logs /app/tmp /app/uploads",
    "chown -R www-data:www-data /app",
    "ln -sf /usr/bin/python3 /usr/local/bin/python",
    "pip3 install -r /app/requirements.txt",
]
on_start = [
    "service nginx start",
    "service postgresql start",
]
on_stop = [
    "nginx -s stop",
    "pg_ctl stop -D /var/lib/postgresql/data",
]
```

### Event yang Didukung

| Field | Kapan dieksekusi | Keterangan |
|-------|-----------------|------------|
| `on_create` | Step 7 dari `--up` | Dijalankan setelah seluruh dependency terinstal dan volume terkonfigurasi. Gunakan untuk inisialisasi satu kali: setup direktori, inisialisasi database, dll. |
| `on_start` | Dijalankan saat `--up` jika container sudah ada | Gunakan untuk memulai service yang tidak autostart. |
| `on_stop` | Dijalankan saat `--down` | Graceful shutdown untuk service yang berjalan. |

### Format Perintah

Setiap perintah dijalankan secara berurutan menggunakan `lxc-attach -- sh -c "<command>"`. Urutan eksekusi **dijamin** sesuai urutan penulisan di file `.mel`.

Karena dieksekusi melalui `sh -c`, perintah mendukung:
- Piping: `"cat /etc/hosts | grep localhost"`
- Redirection: `"echo 'config' >> /etc/app.conf"`
- Chaining: `"apt-get install -y curl && curl -sSL https://... | bash"`

Jika salah satu perintah gagal (exit code non-zero), Engine mencatat error dan melanjutkan ke perintah berikutnya — lifecycle hooks tidak bersifat fatal.

---

## Section `[services]`

**Status:** Opsional

Mendefinisikan service-service yang berjalan di dalam container. Setiap service didefinisikan sebagai inline table TOML.

```toml
[services]
web    = { command = "gunicorn -w 4 -b 0.0.0.0:8080 app:app", working_dir = "/app", enabled = true  }
worker = { command = "python3 -m celery worker -A tasks",      working_dir = "/app", enabled = false }
cron   = { command = "python3 scheduler.py",                   working_dir = "/app", enabled = true  }
```

### Field Service

| Field | Tipe | Keterangan |
|-------|------|------------|
| `command` | string | Perintah lengkap untuk menjalankan service ini. |
| `working_dir` | string | Direktori kerja saat perintah dieksekusi. |
| `enabled` | boolean | `true` = service aktif; `false` = service didefinisikan tapi tidak dijalankan secara otomatis. |

### Catatan Penting

Section `[services]` saat ini bersifat **informational dan reference** untuk operator. Perintah `--up` tidak secara otomatis menjalankan service yang `enabled = true` sebagai background daemon. Untuk menjalankan service, gunakan `melisa --send <container> <command>` atau tambahkan perintah eksekusi ke `on_create` atau `on_start` di section `[lifecycle]`.

Definisi service tetap berguna sebagai dokumentasi terpusat tentang cara menjalankan tiap komponen aplikasi.

---

## Section `[health]`

**Status:** Opsional

Mendefinisikan health check yang dijalankan setelah deployment selesai untuk memverifikasi bahwa aplikasi berjalan dengan benar.

```toml
[health]
command  = "curl -sf http://localhost:8080/health"
interval = 5
retries  = 3
timeout  = 10
```

| Field | Tipe | Default | Keterangan |
|-------|------|---------|------------|
| `command` | string | — | Perintah shell yang dieksekusi di dalam container. Exit code `0` = sehat, non-zero = gagal. |
| `interval` | integer | `5` | Detik antara percobaan ulang jika health check gagal. |
| `retries` | integer | `3` | Jumlah maksimum percobaan sebelum dinyatakan gagal. |
| `timeout` | integer | `10` | Timeout per percobaan dalam detik. |

### Perilaku Health Check

Health check dijalankan di akhir Step 7. Engine melakukan retry dengan backoff linear:

```
Percobaan 1 → tunggu 5 detik
Percobaan 2 → tunggu 5 detik
Percobaan 3 → [FAILED] Deployment selesai tapi aplikasi tidak merespons
```

Kegagalan health check **tidak** membatalkan deployment — container dan semua konfigurasinya tetap ada. Pesan error memberitahu operator bahwa aplikasi perlu diperiksa.

Contoh output saat health check berhasil:

```
[STEP 7/7]  Running lifecycle hooks (on_create)
[INFO] Health check: curl -sf http://localhost:8080/health
[SUCCESS] Application is healthy and responding.
```

---

## Contoh Lengkap: Aplikasi Flask + PostgreSQL

```toml
# ── PROJECT ───────────────────────────────────────────────────────────────────
[project]
name        = "flask-api"
version     = "2.1.0"
description = "REST API dengan Flask dan PostgreSQL"
author      = "Bob <bob@example.com>"

# ── CONTAINER ─────────────────────────────────────────────────────────────────
[container]
distro     = "ubuntu/jammy/amd64"
name       = "flask-api-prod"
auto_start = true

# ── ENV ───────────────────────────────────────────────────────────────────────
[env]
APP_PORT    = "5000"
APP_ENV     = "production"
DB_HOST     = "localhost"
DB_PORT     = "5432"
DB_NAME     = "flask_db"
DB_USER     = "flask_user"

# ── DEPENDENCIES ──────────────────────────────────────────────────────────────
[dependencies]
apt = ["python3", "python3-pip", "postgresql", "postgresql-client", "curl"]
pip = ["flask", "gunicorn", "psycopg2-binary", "flask-sqlalchemy", "python-dotenv"]

# ── VOLUMES ───────────────────────────────────────────────────────────────────
[volumes]
mounts = [
    "/home/bob/flask-api:/app",
    "/var/backups/flask-api:/backups",
]

# ── PORTS ─────────────────────────────────────────────────────────────────────
[ports]
expose = ["5000", "5432"]

# ── LIFECYCLE ─────────────────────────────────────────────────────────────────
[lifecycle]
on_create = [
    "mkdir -p /app/logs",
    "chown -R www-data:www-data /app",
    "service postgresql start",
    "su - postgres -c \"psql -c \\\"CREATE USER flask_user WITH PASSWORD 'password';\\\"\"",
    "su - postgres -c \"psql -c \\\"CREATE DATABASE flask_db OWNER flask_user;\\\"\"",
]
on_stop = [
    "service postgresql stop",
]

# ── SERVICES ──────────────────────────────────────────────────────────────────
[services]
api = { command = "gunicorn -w 4 -b 0.0.0.0:5000 app:app", working_dir = "/app", enabled = true }

# ── HEALTH CHECK ──────────────────────────────────────────────────────────────
[health]
command  = "curl -sf http://localhost:5000/api/health"
interval = 5
retries  = 3
timeout  = 10
```

---

## Contoh Lengkap: Node.js Minimal

```toml
[project]
name = "node-api"

[container]
distro = "alpine/3.18/amd64"

[dependencies]
apk = ["nodejs", "npm"]
npm = ["pm2"]

[lifecycle]
on_create = [
    "npm install --prefix /app",
]

[health]
command = "curl -sf http://localhost:3000/health"
```

---

## Validasi dan Error Umum

### Error: `distro` tidak ditemukan

```
[ERROR] Distro 'ubuntu/invalid/amd64' tidak ditemukan di daftar distribusi.
Jalankan `melisa --search ubuntu` untuk melihat opsi yang valid.
```

Pastikan format `name/release/arch` tepat dan distribusi tersebut tersedia di image server LXC.

### Error: `[project].name` kosong

TOML tidak akan mem-parse field string kosong sebagai valid. Pastikan `name` selalu diisi.

### Warning: Package manager tidak cocok

```
[WARNING] Container menggunakan 'apk' tapi hanya field 'apt' yang diisi.
Tidak ada paket sistem yang akan diinstal.
```

Sesuaikan field dependency dengan package manager distribusi yang dipilih.

### Perintah `on_create` gagal

```
[ERROR] Lifecycle hook gagal: "pip3 install -r /app/requirements.txt" (exit code 1)
[INFO]  Melanjutkan ke hook berikutnya...
```

Kegagalan lifecycle hook tidak menghentikan deployment. Periksa apakah path file sudah benar dan volume sudah ter-mount sebelum perintah dijalankan.

---

## Referensi Tipe Data Internal

Struct Rust yang merepresentasikan file `.mel` (dari `src/deployment/mel_parser.rs`):

```rust
pub struct MelManifest {
    pub project:      ProjectSection,
    pub container:    ContainerSection,
    pub env:          HashMap<String, String>,
    pub dependencies: DependencySection,
    pub volumes:      VolumeSection,
    pub ports:        PortSection,
    pub lifecycle:    LifecycleSection,
    pub services:     HashMap<String, ServiceDefinition>,
    pub health:       Option<HealthSection>,
}

pub struct ContainerSection {
    pub distro:     String,
    pub name:       Option<String>,
    pub auto_start: bool,
}

pub struct DependencySection {
    pub apt:      Vec<String>,
    pub dnf:      Vec<String>,
    pub apk:      Vec<String>,
    pub pacman:   Vec<String>,
    pub pip:      Vec<String>,
    pub npm:      Vec<String>,
    pub cargo:    Vec<String>,
    pub gem:      Vec<String>,
    pub composer: Vec<String>,
}

pub struct HealthSection {
    pub command:  String,
    pub interval: Option<u64>,
    pub retries:  Option<u32>,
    pub timeout:  Option<u64>,
}
```