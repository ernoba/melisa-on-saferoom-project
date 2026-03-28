# Menulis Rust Tests

Test Rust MELISA berada di `src/deployment/tests.rs`. File ini diorganisasi menjadi empat modul test yang mencerminkan empat lapisan deployment pipeline.

---

## Menjalankan Tests

```bash
# Jalankan semua test Rust
cargo test

# Jalankan satu modul test
cargo test tests_mel_parser

# Jalankan satu test spesifik
cargo test test_valid_manifest_parses_correctly

# Tampilkan output println! saat test berjalan
cargo test -- --nocapture

# Jalankan test secara paralel (default) atau serial
cargo test -- --test-threads=1
```

---

## Struktur Modul Test

```
src/deployment/tests.rs
├── mod tests_mel_parser        ← Pengujian mel_parser.rs
│   ├── Helper: write_temp_mel()
│   ├── Helper: make_valid_manifest()
│   ├── Helper: make_manifest_with_name()
│   └── Helper: make_manifest_with_distro()
├── mod tests_dependency        ← Pengujian dependency.rs
├── mod tests_deployer          ← Pengujian deployer.rs
└── mod tests_integration       ← Pipeline end-to-end
    └── Helper: write_temp()
```

File ini di-include dari `src/deployment/mod.rs`:

```rust
// src/deployment/mod.rs
pub mod mel_parser;
pub mod deployer;
pub mod dependency;
#[cfg(test)]
mod tests;
```

---

## Modul 1: `tests_mel_parser`

Menguji parsing TOML → `MelManifest` struct.

### Import yang Dibutuhkan

```rust
#[cfg(test)]
mod tests_mel_parser {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use std::collections::HashMap;
    use crate::deployment::mel_parser::{
        load_mel_file,
        MelManifest,
        ProjectSection,
        ContainerSection,
        DependencySection,
        VolumeSection,
        PortSection,
        LifecycleSection,
    };

    // Helper: tulis konten .mel ke file sementara
    fn write_temp_mel(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }
```

### Pola Test Parser

Semua test parser mengikuti pola yang sama: tulis konten TOML ke file sementara, parse dengan `load_mel_file`, assert hasilnya.

```rust
    #[tokio::test]
    async fn test_valid_manifest_parses_correctly() {
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"

[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;

        assert!(result.is_ok(), "Manifest valid harus berhasil di-parse");
        let m = result.unwrap();
        assert_eq!(m.project.name, "my-app");
        assert_eq!(m.project.version.as_deref(), Some("1.0.0"));
        assert_eq!(m.container.distro, "ubuntu/jammy/amd64");
    }
```

### Menguji Error Parsing

```rust
    #[tokio::test]
    async fn test_missing_required_field_returns_error() {
        // [project].name wajib diisi
        let content = r#"
[project]
version = "1.0.0"

[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;
        assert!(result.is_err(), "Manifest tanpa project.name harus gagal");
    }

    #[tokio::test]
    async fn test_invalid_toml_returns_error() {
        let content = "ini bukan toml yang valid [[[";
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;
        assert!(result.is_err(), "TOML tidak valid harus return Err");
    }
```

### Menguji Nilai Default

```rust
    #[tokio::test]
    async fn test_auto_start_defaults_to_true() {
        let content = r#"
[project]
name = "test-app"
[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp_mel(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();
        assert!(m.container.auto_start, "auto_start harus default true");
    }

    #[tokio::test]
    async fn test_health_section_optional() {
        let content = r#"
[project]
name = "no-health"
[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp_mel(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();
        assert!(m.health.is_none(), "health section harus opsional");
    }
```

### Menguji `effective_name()`

Method `effective_name()` melakukan normalisasi nama: spasi → `-`, semua lowercase.

```rust
    #[test]
    fn test_effective_name_uses_container_name_when_set() {
        let mut m = make_valid_manifest();
        m.container.name = Some("custom-container".to_string());
        let name = m.container.effective_name(&m.project.name);
        assert_eq!(name, "custom-container");
    }

    #[test]
    fn test_effective_name_normalizes_project_name() {
        let mut m = make_valid_manifest();
        m.container.name = None;
        m.project.name = "My Web App".to_string();
        let name = m.container.effective_name(&m.project.name);
        assert_eq!(name, "my-web-app");
        assert!(!name.contains(' '), "Nama tidak boleh mengandung spasi");
    }
```

### Helper Builder

Gunakan helper builders untuk membuat `MelManifest` yang sudah valid sebagai base untuk test yang hanya perlu mengubah satu field:

```rust
    fn make_valid_manifest() -> MelManifest {
        MelManifest {
            project: ProjectSection {
                name: "test-app".to_string(),
                version: None,
                description: None,
                author: None,
            },
            container: ContainerSection {
                distro: "ubuntu/jammy/amd64".to_string(),
                name: None,
                auto_start: true,
            },
            env: HashMap::new(),
            dependencies: DependencySection::default(),
            ports: PortSection::default(),
            volumes: VolumeSection::default(),
            lifecycle: LifecycleSection::default(),
            services: HashMap::new(),
            health: None,
        }
    }

    fn make_manifest_with_distro(distro: &str) -> MelManifest {
        let mut m = make_valid_manifest();
        m.container.distro = distro.to_string();
        m
    }
```

---

## Modul 2: `tests_dependency`

Menguji `dependency.rs` — pembangunan perintah instalasi.

### Import

```rust
#[cfg(test)]
mod tests_dependency {
    use crate::deployment::dependency::{
        build_system_install_cmd,
        build_update_cmd,
        has_lang_deps,
    };
    use crate::deployment::mel_parser::DependencySection;
```

### Menguji `build_update_cmd`

```rust
    #[test]
    fn test_build_update_cmd_apt() {
        let cmd = build_update_cmd("apt-get");
        assert!(cmd.contains("apt-get"), "Harus pakai apt-get");
        assert!(cmd.contains("update"), "Harus ada perintah update");
    }

    #[test]
    fn test_build_update_cmd_apk() {
        let cmd = build_update_cmd("apk");
        assert!(cmd.contains("apk update"));
    }

    #[test]
    fn test_build_update_cmd_pacman() {
        let cmd = build_update_cmd("pacman");
        assert!(cmd.contains("pacman") && cmd.contains("-Sy"));
    }
```

### Menguji `build_system_install_cmd`

```rust
    #[test]
    fn test_build_install_cmd_apt_with_packages() {
        let mut deps = DependencySection::default();
        deps.apt = vec!["curl".to_string(), "git".to_string()];

        let cmd = build_system_install_cmd("apt-get", &deps);

        assert!(cmd.is_some(), "Harus menghasilkan command jika ada packages");
        let cmd = cmd.unwrap();
        assert!(cmd.contains("apt-get install -y"));
        assert!(cmd.contains("curl") && cmd.contains("git"));
    }

    #[test]
    fn test_build_install_cmd_returns_none_when_empty() {
        let deps = DependencySection::default();
        let cmd = build_system_install_cmd("apt-get", &deps);
        assert!(cmd.is_none(), "Tidak ada packages → tidak ada command");
    }

    #[test]
    fn test_build_install_cmd_unknown_pm_returns_none() {
        let mut deps = DependencySection::default();
        deps.apt = vec!["curl".to_string()];
        let cmd = build_system_install_cmd("homebrew", &deps);
        assert!(cmd.is_none(), "Package manager tidak dikenal harus return None");
    }
```

### Menguji `has_lang_deps`

```rust
    #[test]
    fn test_has_lang_deps_false_when_all_empty() {
        let deps = DependencySection::default();
        assert!(!has_lang_deps(&deps));
    }

    #[test]
    fn test_has_lang_deps_true_when_pip_present() {
        let mut deps = DependencySection::default();
        deps.pip = vec!["flask".to_string()];
        assert!(has_lang_deps(&deps));
    }

    #[test]
    fn test_has_lang_deps_true_when_npm_present() {
        let mut deps = DependencySection::default();
        deps.npm = vec!["express".to_string()];
        assert!(has_lang_deps(&deps));
    }
```

---

## Modul 3: `tests_deployer`

Menguji fungsi-fungsi publik di `deployer.rs`.

### Import

```rust
#[cfg(test)]
mod tests_deployer {
    use crate::deployment::deployer::{
        build_env_inject_cmd,
        build_health_check_retry_plan,
        format_ports_summary,
        format_volumes_summary,
    };
    use crate::deployment::mel_parser::HealthSection;
```

### Menguji `build_env_inject_cmd`

```rust
    #[test]
    fn test_env_inject_cmd_structure() {
        let cmd = build_env_inject_cmd("APP_PORT", "8080");
        assert!(cmd.contains("APP_PORT"));
        assert!(cmd.contains("8080"));
        assert!(cmd.contains("/etc/environment"));
        assert!(cmd.contains("sed"), "Harus pakai sed untuk hapus entry lama");
    }

    #[test]
    fn test_env_inject_handles_special_chars() {
        let cmd = build_env_inject_cmd("DB_URL", "postgres://user:pass@localhost/db");
        assert!(cmd.contains("DB_URL"));
        assert!(cmd.contains("postgres://user:pass@localhost/db"));
    }
```

### Menguji `build_health_check_retry_plan`

```rust
    #[test]
    fn test_health_check_default_values() {
        let h = HealthSection {
            command:  "curl localhost".to_string(),
            interval: None,
            retries:  None,
            timeout:  None,
        };
        let plan = build_health_check_retry_plan(&h);
        assert_eq!(plan.retries, 3, "Default retries = 3");
        assert_eq!(plan.interval_secs, 5, "Default interval = 5 detik");
        assert_eq!(plan.command, "curl localhost");
    }

    #[test]
    fn test_health_check_custom_values() {
        let h = HealthSection {
            command:  "wget -q localhost:9000".to_string(),
            interval: Some(10),
            retries:  Some(5),
            timeout:  Some(30),
        };
        let plan = build_health_check_retry_plan(&h);
        assert_eq!(plan.retries, 5);
        assert_eq!(plan.interval_secs, 10);
    }
```

---

## Modul 4: `tests_integration`

Menguji pipeline multi-langkah yang mewakili alur kerja `--up` yang nyata.

### Pola Test Integrasi

```rust
#[cfg(test)]
mod tests_integration {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use crate::deployment::mel_parser::load_mel_file;
    use crate::deployment::dependency::{build_system_install_cmd, has_lang_deps};

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }
```

### Contoh: Pipeline Parse → Build Command

```rust
    #[tokio::test]
    async fn test_parse_then_build_install_cmd() {
        let content = r#"
[project]
name = "pipeline-test"
[container]
distro = "ubuntu/jammy/amd64"
[dependencies]
apt = ["nodejs", "npm", "git"]
pip = ["flask"]
"#;
        let f = write_temp(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();

        // Simulasi apa yang deployer.rs lakukan
        let sys_cmd = build_system_install_cmd("apt-get", &m.dependencies);
        assert!(sys_cmd.is_some());
        let cmd = sys_cmd.unwrap();
        assert!(cmd.contains("nodejs") && cmd.contains("npm") && cmd.contains("git"));
        assert!(has_lang_deps(&m.dependencies), "pip diisi → has_lang_deps = true");
    }
```

### Contoh: Urutan Lifecycle Hooks

```rust
    #[tokio::test]
    async fn test_lifecycle_hooks_order_preserved() {
        let content = r#"
[project]
name = "hook-test"
[container]
distro = "alpine/3.18/amd64"
[lifecycle]
on_create = ["step-1", "step-2", "step-3"]
on_stop   = ["cleanup"]
"#;
        let f = write_temp(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();

        // Urutan HARUS preserved
        assert_eq!(m.lifecycle.on_create[0], "step-1");
        assert_eq!(m.lifecycle.on_create[1], "step-2");
        assert_eq!(m.lifecycle.on_create[2], "step-3");
        assert_eq!(m.lifecycle.on_stop[0], "cleanup");
        assert!(m.lifecycle.on_start.is_empty(), "on_start tidak diisi → kosong");
    }
```

---

## Checklist sebelum Submit

Sebelum PR, pastikan semua test berikut lolos:

```bash
# Tidak ada compile error
cargo build

# Semua tests pass
cargo test

# Formatting (CI akan menolak jika tidak sesuai)
cargo fmt --check

# Tidak ada clippy warnings
cargo clippy -- -D warnings
```

---

## Tips dan Best Practices

**Gunakan `r#"..."#` untuk konten TOML** — raw string literal menghindari masalah escape di konten TOML.

**Selalu assert pesan error yang bermakna** — argumen kedua `assert!` dan `assert_eq!` adalah pesan yang muncul saat test gagal. Tulis pesan yang menjelaskan *apa yang seharusnya terjadi*.

```rust
// Buruk
assert!(result.is_ok());

// Bagus
assert!(result.is_ok(), "Manifest dengan semua field valid harus berhasil di-parse, bukan: {:?}", result);
```

**Gunakan `#[tokio::test]` untuk fungsi async** — `load_mel_file` adalah async, jadi test yang memanggilnya harus didekorasi dengan `#[tokio::test]`, bukan `#[test]` biasa.

**Gunakan `DependencySection::default()` untuk boilerplate** — struct ini mengimplementasikan `Default` sehingga semua field Vec dimulai sebagai empty.

**Satu test, satu konsep** — jangan menguji terlalu banyak hal dalam satu test. Jika test gagal, nama test harus langsung mengindikasikan apa yang rusak.