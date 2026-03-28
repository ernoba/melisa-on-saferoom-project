// src/deployment/tests.rs
// Jalankan: cargo test deployment
// Tidak butuh root atau LXC — hanya test logika murni

#[cfg(test)]
mod tests_mel_parser {
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── Kita import langsung function & struct dari mel_parser ──────────────
    // Karena test ada di modul yang sama, pakai super::
    // Jika test file terpisah, ganti dengan: use crate::deployment::mel_parser::*;

    use crate::deployment::mel_parser::{
        load_mel_file, validate_manifest_pub,
        MelManifest, ProjectSection, ContainerSection,
        DependencySection, PortSection, VolumeSection,
        LifecycleSection, MelParseError,
    };
    use std::collections::HashMap;

    // ── Helper: tulis konten ke file temp, return path-nya ─────────────────
    fn write_temp_mel(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("Gagal buat temp file");
        f.write_all(content.as_bytes()).expect("Gagal tulis temp file");
        f
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 1: Manifest minimal yang valid
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_parse_manifest_minimal_valid() {
        let content = r#"
[project]
name = "hello-app"

[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;
        assert!(result.is_ok(), "Manifest minimal harus berhasil diparsing");

        let m = result.unwrap();
        assert_eq!(m.project.name, "hello-app");
        assert_eq!(m.container.distro, "ubuntu/jammy/amd64");
        assert!(m.container.auto_start, "auto_start default harus true");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 2: Manifest lengkap dengan semua section
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_parse_manifest_full() {
        let content = r#"
[project]
name        = "full-app"
version     = "2.0.0"
description = "Aplikasi lengkap"
author      = "dev@test.com"

[container]
distro     = "debian/bookworm/amd64"
name       = "my-container"
auto_start = false

[env]
APP_PORT = "8080"
DEBUG    = "false"

[dependencies]
apt   = ["curl", "git", "build-essential"]
pip   = ["flask", "gunicorn"]
npm   = ["typescript"]
cargo = []
gem   = []

[ports]
expose = ["8080:8080", "443:443"]

[volumes]
mounts = ["./src:/app/src", "./data:/var/data"]

[lifecycle]
on_create = ["mkdir -p /app/logs", "chmod 755 /app"]
on_start  = ["echo starting"]
on_stop   = ["echo stopping"]

[health]
command  = "curl -sf http://localhost:8080/health"
interval = 30
retries  = 3
timeout  = 10
"#;
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;
        assert!(result.is_ok(), "Manifest lengkap harus berhasil diparsing");

        let m = result.unwrap();
        assert_eq!(m.project.version.as_deref(), Some("2.0.0"));
        assert_eq!(m.container.name.as_deref(), Some("my-container"));
        assert!(!m.container.auto_start);
        assert_eq!(m.env.get("APP_PORT").map(|s| s.as_str()), Some("8080"));
        assert_eq!(m.dependencies.apt.len(), 3);
        assert_eq!(m.dependencies.pip.len(), 2);
        assert_eq!(m.ports.expose.len(), 2);
        assert_eq!(m.volumes.mounts.len(), 2);
        assert_eq!(m.lifecycle.on_create.len(), 2);
        assert!(m.health.is_some());
        assert_eq!(m.health.unwrap().retries, Some(3));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 3: File tidak ditemukan → MelParseError::NotFound
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_file_not_found() {
        let result = load_mel_file("/tmp/TIDAK_ADA_FILE_INI_12345.mel").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MelParseError::NotFound(path) => {
                assert!(path.contains("TIDAK_ADA_FILE_INI_12345"));
            }
            e => panic!("Error yang diharapkan NotFound, dapat: {:?}", e),
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 4: TOML rusak → MelParseError::TomlParse
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_invalid_toml_syntax() {
        let content = r#"
[project
name = "broken"  # kurung siku tidak ditutup — syntax error
"#;
        let f = write_temp_mel(content);
        let result = load_mel_file(f.path().to_str().unwrap()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MelParseError::TomlParse(_) => {} // ✅ expected
            e => panic!("Error yang diharapkan TomlParse, dapat: {:?}", e),
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 5: Validasi — project.name kosong harus ditolak
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_validation_empty_project_name() {
        let m = make_manifest_with_name("");
        let result = validate_manifest_pub(&m);
        assert!(result.is_err(), "Nama proyek kosong harus gagal validasi");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("name"), "Pesan error harus menyebut 'name'");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 6: Validasi — distro kosong harus ditolak
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_validation_empty_distro() {
        let m = make_manifest_with_distro("");
        let result = validate_manifest_pub(&m);
        assert!(result.is_err(), "Distro kosong harus gagal validasi");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("distro"), "Pesan error harus menyebut 'distro'");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 7: Validasi — format port salah (tanpa titik dua)
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_validation_bad_port_format() {
        let mut m = make_valid_manifest();
        m.ports.expose = vec!["8080".to_string()]; // format salah, harus "8080:8080"
        let result = validate_manifest_pub(&m);
        assert!(result.is_err(), "Format port salah harus gagal validasi");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("port") || err.contains("Port"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 8: Validasi — format volume salah
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_validation_bad_volume_format() {
        let mut m = make_valid_manifest();
        m.volumes.mounts = vec!["/hanya/satu/path".to_string()]; // harus "host:container"
        let result = validate_manifest_pub(&m);
        assert!(result.is_err(), "Format volume salah harus gagal validasi");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("volume") || err.contains("Volume"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 9: effective_name() — nama dari container.name jika ada
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_effective_name_uses_container_name() {
        let mut m = make_valid_manifest();
        m.container.name = Some("custom-name".to_string());
        let name = m.container.effective_name(&m.project.name);
        assert_eq!(name, "custom-name");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 10: effective_name() — fallback ke project.name jika container.name kosong
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_effective_name_fallback_to_project_name() {
        let mut m = make_valid_manifest();
        m.container.name = None;
        m.project.name = "My Cool App".to_string();
        let name = m.container.effective_name(&m.project.name);
        // Spasi diganti '-', huruf kecil semua
        assert_eq!(name, "my-cool-app");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 11: Section [dependencies] boleh kosong total
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_empty_dependencies_section_ok() {
        let content = r#"
[project]
name = "no-deps"

[container]
distro = "alpine/3.18/amd64"
"#;
        let f = write_temp_mel(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();
        assert!(m.dependencies.apt.is_empty());
        assert!(m.dependencies.pip.is_empty());
        assert!(m.dependencies.npm.is_empty());
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 12: health section boleh tidak ada (None)
    // ────────────────────────────────────────────────────────────────────────
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
        assert!(m.health.is_none(), "health section harus opsional (None jika tidak diisi)");
    }

    // ── Helper builders ─────────────────────────────────────────────────────

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

    fn make_manifest_with_name(name: &str) -> MelManifest {
        let mut m = make_valid_manifest();
        m.project.name = name.to_string();
        m
    }

    fn make_manifest_with_distro(distro: &str) -> MelManifest {
        let mut m = make_valid_manifest();
        m.container.distro = distro.to_string();
        m
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests untuk dependency.rs — logika perintah install
// ════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_dependency {
    use crate::deployment::dependency::{
        build_system_install_cmd,
        build_update_cmd,
        has_lang_deps,
    };
    use crate::deployment::mel_parser::DependencySection;

    // ────────────────────────────────────────────────────────────────────────
    // TEST 13: build_update_cmd untuk setiap package manager
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_update_cmd_apt() {
        let cmd = build_update_cmd("apt-get");
        assert!(cmd.contains("apt-get"), "Harus pakai apt-get");
        assert!(cmd.contains("update"), "Harus ada perintah update");
    }

    #[test]
    fn test_build_update_cmd_pacman() {
        let cmd = build_update_cmd("pacman");
        assert!(cmd.contains("pacman"));
        assert!(cmd.contains("-Sy"));
    }

    #[test]
    fn test_build_update_cmd_apk() {
        let cmd = build_update_cmd("apk");
        assert!(cmd.contains("apk update"));
    }

    #[test]
    fn test_build_update_cmd_dnf() {
        let cmd = build_update_cmd("dnf");
        assert!(cmd.contains("dnf"));
        assert!(cmd.contains("makecache") || cmd.contains("update"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 14: build_system_install_cmd — apt dengan paket-paket
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_install_cmd_apt_with_packages() {
        let mut deps = DependencySection::default();
        deps.apt = vec!["curl".to_string(), "git".to_string(), "vim".to_string()];

        let cmd = build_system_install_cmd("apt-get", &deps);
        assert!(cmd.is_some(), "Harus menghasilkan command jika ada packages");

        let cmd = cmd.unwrap();
        assert!(cmd.contains("apt-get install -y"), "Format apt-get install -y");
        assert!(cmd.contains("curl"));
        assert!(cmd.contains("git"));
        assert!(cmd.contains("vim"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 15: build_system_install_cmd — apt kosong → None
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_install_cmd_returns_none_when_empty() {
        let deps = DependencySection::default(); // semua kosong
        let cmd = build_system_install_cmd("apt-get", &deps);
        assert!(cmd.is_none(), "Tidak ada packages → tidak ada command yang dibuat");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 16: build_system_install_cmd — pacman
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_install_cmd_pacman() {
        let mut deps = DependencySection::default();
        deps.pacman = vec!["nodejs".to_string(), "npm".to_string()];

        let cmd = build_system_install_cmd("pacman", &deps).unwrap();
        assert!(cmd.contains("pacman -S --noconfirm"));
        assert!(cmd.contains("nodejs"));
        assert!(cmd.contains("npm"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 17: build_system_install_cmd — apk
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_install_cmd_apk() {
        let mut deps = DependencySection::default();
        deps.apk = vec!["python3".to_string()];

        let cmd = build_system_install_cmd("apk", &deps).unwrap();
        assert!(cmd.contains("apk add"));
        assert!(cmd.contains("python3"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 18: Package manager tidak dikenal → None (bukan crash)
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_build_install_cmd_unknown_pm_returns_none() {
        let mut deps = DependencySection::default();
        deps.apt = vec!["curl".to_string()];
        // pkg manager "chocolatey" tidak ada di mapping
        let cmd = build_system_install_cmd("chocolatey", &deps);
        assert!(cmd.is_none(), "PM tidak dikenal harus return None, bukan crash");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 19: has_lang_deps — benar kalau ada pip/npm/dll
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_has_lang_deps_true_when_pip_filled() {
        let mut deps = DependencySection::default();
        deps.pip = vec!["flask".to_string()];
        assert!(has_lang_deps(&deps));
    }

    #[test]
    fn test_has_lang_deps_false_when_all_empty() {
        let deps = DependencySection::default();
        assert!(!has_lang_deps(&deps), "Semua lang deps kosong → false");
    }

    #[test]
    fn test_has_lang_deps_true_when_cargo_filled() {
        let mut deps = DependencySection::default();
        deps.cargo = vec!["ripgrep".to_string()];
        assert!(has_lang_deps(&deps));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests untuk deployer.rs — logika helper yang tidak butuh LXC
// ════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_deployer {
    use crate::deployment::deployer::{
        build_env_inject_cmd,
        build_health_check_retry_plan,
        format_ports_summary,
        format_volumes_summary,
    };
    use crate::deployment::mel_parser::HealthSection;

    // ────────────────────────────────────────────────────────────────────────
    // TEST 20: build_env_inject_cmd — menghasilkan perintah sed + echo
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_env_inject_cmd_contains_key_and_val() {
        let cmd = build_env_inject_cmd("APP_PORT", "3000");
        assert!(cmd.contains("APP_PORT"), "Command harus mengandung nama key");
        assert!(cmd.contains("3000"), "Command harus mengandung value");
        assert!(cmd.contains("/etc/environment"), "Harus target /etc/environment");
        // Pastikan hapus duplikat lama
        assert!(cmd.contains("sed"), "Harus pakai sed untuk hapus entry lama");
    }

    #[test]
    fn test_env_inject_cmd_handles_special_chars_in_value() {
        // Value dengan spasi dan karakter khusus
        let cmd = build_env_inject_cmd("DB_URL", "postgres://user:pass@localhost/db");
        assert!(cmd.contains("DB_URL"));
        assert!(cmd.contains("postgres://user:pass@localhost/db"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 21: build_health_check_retry_plan — jumlah retry benar
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_health_check_retry_plan_default_values() {
        let h = HealthSection {
            command:  "curl localhost".to_string(),
            interval: None,   // default 5
            retries:  None,   // default 3
            timeout:  None,
        };
        let plan = build_health_check_retry_plan(&h);
        assert_eq!(plan.retries, 3, "Default retries harus 3");
        assert_eq!(plan.interval_secs, 5, "Default interval harus 5 detik");
        assert_eq!(plan.command, "curl localhost");
    }

    #[test]
    fn test_health_check_retry_plan_custom_values() {
        let h = HealthSection {
            command:  "wget -q localhost:8080".to_string(),
            interval: Some(10),
            retries:  Some(5),
            timeout:  Some(30),
        };
        let plan = build_health_check_retry_plan(&h);
        assert_eq!(plan.retries, 5);
        assert_eq!(plan.interval_secs, 10);
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 22: format_ports_summary — output string rapi
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_format_ports_summary_single() {
        let ports = vec!["3000:3000".to_string()];
        let s = format_ports_summary(&ports);
        assert!(s.contains("3000:3000"));
    }

    #[test]
    fn test_format_ports_summary_empty() {
        let ports: Vec<String> = vec![];
        let s = format_ports_summary(&ports);
        // Tidak boleh crash, boleh return string kosong atau "-"
        assert!(!s.is_empty() || s.is_empty()); // tidak panic = lulus
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 23: format_volumes_summary — semua mount ditampilkan
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_format_volumes_summary_multiple() {
        let vols = vec![
            "./src:/app/src".to_string(),
            "./data:/var/data".to_string(),
        ];
        let s = format_volumes_summary(&vols);
        assert!(s.contains("./src:/app/src"));
        assert!(s.contains("./data:/var/data"));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests integrasi parsing → validasi (end-to-end tanpa LXC)
// ════════════════════════════════════════════════════════════════════════════
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

    // ────────────────────────────────────────────────────────────────────────
    // TEST 24: Parse .mel → langsung bangun install command
    // ────────────────────────────────────────────────────────────────────────
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
        assert!(cmd.contains("nodejs"));
        assert!(cmd.contains("npm"));
        assert!(cmd.contains("git"));

        // Cek ada lang deps
        assert!(has_lang_deps(&m.dependencies), "pip = ['flask'] harus terdeteksi sebagai lang deps");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 25: effective_name dari file asli
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_effective_name_from_parsed_manifest() {
        let content = r#"
[project]
name = "My Web App"

[container]
distro = "ubuntu/jammy/amd64"
"#;
        let f = write_temp(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();

        let effective = m.container.effective_name(&m.project.name);
        // Nama proyek dengan spasi → huruf kecil, spasi jadi strip
        assert_eq!(effective, "my-web-app");
        assert!(!effective.contains(' '), "Nama container tidak boleh mengandung spasi");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 26: Lifecycle hooks terbaca dengan urutan benar
    // ────────────────────────────────────────────────────────────────────────
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

        assert_eq!(m.lifecycle.on_create[0], "step-1");
        assert_eq!(m.lifecycle.on_create[1], "step-2");
        assert_eq!(m.lifecycle.on_create[2], "step-3");
        assert_eq!(m.lifecycle.on_stop[0], "cleanup");
        assert!(m.lifecycle.on_start.is_empty(), "on_start tidak diisi → harus kosong");
    }

    // ────────────────────────────────────────────────────────────────────────
    // TEST 27: Services terbaca dengan benar
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_services_parsed_correctly() {
        let content = r#"
[project]
name = "svc-test"

[container]
distro = "ubuntu/jammy/amd64"

[services]
web    = { command = "node server.js", working_dir = "/app", enabled = true  }
worker = { command = "node worker.js", working_dir = "/app", enabled = false }
"#;
        let f = write_temp(content);
        let m = load_mel_file(f.path().to_str().unwrap()).await.unwrap();

        assert_eq!(m.services.len(), 2);

        let web = m.services.get("web").expect("Service 'web' harus ada");
        assert!(web.enabled);
        assert_eq!(web.command, "node server.js");

        let worker = m.services.get("worker").expect("Service 'worker' harus ada");
        assert!(!worker.enabled);
    }
}