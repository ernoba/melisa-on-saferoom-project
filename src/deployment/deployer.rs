use tokio::process::Command;
use std::process::Stdio;
use tokio::time::{sleep, Duration};
use crate::cli::color_text::{GREEN, YELLOW, RED, BOLD, RESET, CYAN};
use crate::cli::loading::execute_with_spinner;
use crate::core::container::{
    LXC_PATH, create_new_container, start_container,
    stop_container, add_shared_folder,
};
use crate::distros::distro::get_lxc_distro_list;
use crate::deployment::mel_parser::{MelManifest, load_mel_file, MelParseError};
use crate::deployment::dependency::{
    detect_pkg_manager, install_system_deps,
    install_lang_deps, lxc_exec, lxc_exec_silent,
};

/// Entry point utama: melisa --up <path_ke_file.mel>
pub async fn cmd_up(mel_path: &str, audit: bool) {
    println!("\n{}━━━ MELISA DEPLOYMENT ENGINE ━━━{}", BOLD, RESET);
    println!("{}[UP]{} Membaca manifest: {}{}{}", CYAN, RESET, BOLD, mel_path, RESET);

    // ── 1. Parse manifest ─────────────────────────────────────────────────
    let manifest = match load_mel_file(mel_path).await {
        Ok(m) => m,
        Err(MelParseError::NotFound(p)) => {
            println!("{}[ERROR]{} File '{}' tidak ditemukan.", RED, RESET, p);
            println!("{}Tip:{} Pastikan path benar. Contoh: melisa --up ./myapp/program.mel", YELLOW, RESET);
            return;
        }
        Err(MelParseError::TomlParse(e)) => {
            println!("{}[ERROR]{} File .mel tidak valid:\n  {}", RED, RESET, e);
            return;
        }
        Err(e) => {
            println!("{}[ERROR]{} {}", RED, RESET, e);
            return;
        }
    };

    let container_name = manifest.container.effective_name(&manifest.project.name);
    print_manifest_summary(&manifest, &container_name);

    // ── 2. Cek apakah kontainer sudah ada ─────────────────────────────────
    let already_exists = container_exists(&container_name).await;

    if !already_exists {
        // ── 3. Provision kontainer baru ────────────────────────────────────
        println!("\n{}[STEP 1/7]{} Provisioning kontainer baru...{}", BOLD, RESET, RESET);

        let (distro_list, is_cache) = execute_with_spinner(
            "Memvalidasi distro manifest...",
            |_pb| get_lxc_distro_list(audit),
            audit,
        ).await;

        if is_cache {
            println!("{}[CACHE]{} Menggunakan data distro lokal.", YELLOW, RESET);
        }

        let distro_code = &manifest.container.distro;
        let meta = match distro_list.into_iter().find(|d| &d.slug == distro_code) {
            Some(m) => m,
            None => {
                println!("{}[ERROR]{} Distro '{}' tidak ditemukan.", RED, RESET, distro_code);
                println!("{}Tip:{} Jalankan 'melisa --search' untuk melihat kode yang valid.", YELLOW, RESET);
                return;
            }
        };

        execute_with_spinner(
            &format!("Membuat kontainer '{}'...", container_name),
            |pb| create_new_container(&container_name, meta, pb, audit),
            audit,
        ).await;

    } else {
        println!("{}[INFO]{} Kontainer '{}' sudah ada, melewati provisioning.", YELLOW, RESET, container_name);

        // Pastikan kontainer berjalan
        if !is_container_running(&container_name).await {
            println!("{}[STEP 1/7]{} Menghidupkan kontainer...{}", BOLD, RESET, RESET);
            start_container(&container_name, audit).await;
            wait_for_ready(&container_name).await;
        }
    }

    // ── 4. Deteksi package manager kontainer ──────────────────────────────
    println!("\n{}[STEP 2/7]{} Mendeteksi lingkungan kontainer...{}", BOLD, RESET, RESET);
    let pkg_manager = match detect_pkg_manager(&container_name).await {
        Some(pm) => {
            println!("{}[INFO]{} Package manager terdeteksi: {}{}{}", CYAN, RESET, BOLD, pm, RESET);
            pm
        }
        None => {
            println!("{}[WARNING]{} Package manager tidak terdeteksi. System deps akan dilewati.", YELLOW, RESET);
            String::new()
        }
    };

    // ── 5. Install system dependencies ───────────────────────────────────
    println!("\n{}[STEP 3/7]{} Menginstall system dependencies...{}", BOLD, RESET, RESET);
    if !pkg_manager.is_empty() {
        let ok = install_system_deps(&container_name, &manifest.dependencies, &pkg_manager).await;
        if !ok {
            println!("{}[WARNING]{} Beberapa system deps gagal diinstall, melanjutkan...", YELLOW, RESET);
        }
    }

    // ── 6. Install language dependencies ─────────────────────────────────
    println!("\n{}[STEP 4/7]{} Menginstall language dependencies...{}", BOLD, RESET, RESET);
    let ok = install_lang_deps(&container_name, &manifest.dependencies).await;
    if !ok {
        println!("{}[WARNING]{} Beberapa language deps gagal, melanjutkan...", YELLOW, RESET);
    }

    // ── 7. Mount volumes ──────────────────────────────────────────────────
    println!("\n{}[STEP 5/7]{} Mengatur volumes...{}", BOLD, RESET, RESET);
    for mount in &manifest.volumes.mounts {
        let parts: Vec<&str> = mount.split(':').collect();
        if parts.len() == 2 {
            add_shared_folder(&container_name, parts[0], parts[1]).await;
        }
    }
    if manifest.volumes.mounts.is_empty() {
        println!("{}[INFO]{} Tidak ada volume yang dikonfigurasi.", YELLOW, RESET);
    }

    // ── 8. Inject environment variables ──────────────────────────────────
    println!("\n{}[STEP 6/7]{} Menginjeksi environment variables...{}", BOLD, RESET, RESET);
    if !manifest.env.is_empty() {
        inject_env_vars(&container_name, &manifest.env).await;
    } else {
        println!("{}[INFO]{} Tidak ada env variables.", YELLOW, RESET);
    }

    // ── 9. Jalankan lifecycle on_create hooks ─────────────────────────────
    println!("\n{}[STEP 7/7]{} Menjalankan lifecycle hooks...{}", BOLD, RESET, RESET);
    run_lifecycle_hooks(&container_name, &manifest.lifecycle.on_create, "on_create").await;

    // ── 10. Health check ──────────────────────────────────────────────────
    if let Some(health) = &manifest.health {
        println!("\n{}[HEALTH]{} Menjalankan health check...{}", BOLD, RESET, RESET);
        run_health_check(&container_name, health).await;
    }

    // ── 11. Print ringkasan akhir ─────────────────────────────────────────
    println!("\n{}━━━ DEPLOYMENT SELESAI ━━━{}", GREEN, RESET);
    println!("{}[OK]{} Kontainer '{}{}{}' berhasil di-deploy!", GREEN, RESET, BOLD, container_name, RESET);

    // Tampilkan services yang aktif
    let enabled_services: Vec<_> = manifest.services.iter()
        .filter(|(_, s)| s.enabled)
        .collect();
    if !enabled_services.is_empty() {
        println!("\n{}Services yang dikonfigurasi:{}", BOLD, RESET);
        for (name, svc) in &enabled_services {
            println!("  {}•{} {} → {}", CYAN, RESET, name, svc.command);
        }
        println!("\n{}Tip:{} Gunakan 'melisa --send {} <cmd>' untuk menjalankan service.", YELLOW, RESET, container_name);
    }

    // Tampilkan port
    if !manifest.ports.expose.is_empty() {
        println!("\n{}Ports yang di-expose:{}", BOLD, RESET);
        for port in &manifest.ports.expose {
            println!("  {}•{} {}", CYAN, RESET, port);
        }
    }
    println!();
}

/// Hentikan kontainer dan jalankan on_stop hooks.
pub async fn cmd_down(mel_path: &str, audit: bool) {
    println!("\n{}[DOWN]{} Membaca manifest: {}", CYAN, RESET, mel_path);

    let manifest = match load_mel_file(mel_path).await {
        Ok(m) => m,
        Err(e) => {
            println!("{}[ERROR]{} {}", RED, RESET, e);
            return;
        }
    };

    let container_name = manifest.container.effective_name(&manifest.project.name);

    if !is_container_running(&container_name).await {
        println!("{}[INFO]{} Kontainer '{}' sudah tidak berjalan.", YELLOW, RESET, container_name);
        return;
    }

    // Jalankan on_stop hooks sebelum mematikan
    if !manifest.lifecycle.on_stop.is_empty() {
        println!("{}[INFO]{} Menjalankan on_stop hooks...", CYAN, RESET);
        run_lifecycle_hooks(&container_name, &manifest.lifecycle.on_stop, "on_stop").await;
    }

    stop_container(&container_name, audit).await;
    println!("{}[OK]{} Kontainer '{}' berhasil dihentikan.", GREEN, RESET, container_name);
}

/// Tampilkan info deployment dari file .mel.
pub async fn cmd_mel_info(mel_path: &str) {
    let manifest = match load_mel_file(mel_path).await {
        Ok(m) => m,
        Err(e) => {
            println!("{}[ERROR]{} {}", RED, RESET, e);
            return;
        }
    };
    let container_name = manifest.container.effective_name(&manifest.project.name);
    let running = is_container_running(&container_name).await;

    println!("\n{}━━━ MELISA MANIFEST INFO ━━━{}", BOLD, RESET);
    println!("  {}Proyek   :{} {} v{}", BOLD, RESET,
        manifest.project.name,
        manifest.project.version.as_deref().unwrap_or("?")
    );
    if let Some(desc) = &manifest.project.description {
        println!("  {}Deskripsi:{} {}", BOLD, RESET, desc);
    }
    println!("  {}Kontainer:{} {} ({})", BOLD, RESET, container_name,
        if running { format!("{}RUNNING{}", GREEN, RESET) } else { format!("{}STOPPED{}", RED, RESET) }
    );
    println!("  {}Distro   :{} {}", BOLD, RESET, manifest.container.distro);
    println!("  {}File     :{} {}", BOLD, RESET, mel_path);

    let sys_total = manifest.dependencies.apt.len()
        + manifest.dependencies.pacman.len()
        + manifest.dependencies.dnf.len()
        + manifest.dependencies.apk.len();
    let lang_total = manifest.dependencies.pip.len()
        + manifest.dependencies.npm.len()
        + manifest.dependencies.cargo.len()
        + manifest.dependencies.gem.len()
        + manifest.dependencies.composer.len();

    println!("\n  {}Dependencies:{}", BOLD, RESET);
    println!("    System  : {} paket", sys_total);
    println!("    Language: {} paket", lang_total);

    if !manifest.ports.expose.is_empty() {
        println!("\n  {}Ports:{}", BOLD, RESET);
        for p in &manifest.ports.expose {
            println!("    {}", p);
        }
    }
    if !manifest.volumes.mounts.is_empty() {
        println!("\n  {}Volumes:{}", BOLD, RESET);
        for v in &manifest.volumes.mounts {
            println!("    {}", v);
        }
    }
    if !manifest.services.is_empty() {
        println!("\n  {}Services:{}", BOLD, RESET);
        for (name, svc) in &manifest.services {
            let status = if svc.enabled { format!("{}enabled{}", GREEN, RESET) } else { format!("{}disabled{}", YELLOW, RESET) };
            println!("    {} [{}] → {}", name, status, svc.command);
        }
    }
    println!();
}

// ══════════════════════════ Helper private functions ══════════════════════════

/// Cek apakah kontainer sudah ada di sistem LXC.
async fn container_exists(name: &str) -> bool {
    let out = Command::new("sudo")
        .args(&["lxc-info", "-P", LXC_PATH, "-n", name, "-s"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    out.map(|s| s.success()).unwrap_or(false)
}

/// Cek apakah kontainer sedang berjalan.
async fn is_container_running(name: &str) -> bool {
    let out = Command::new("sudo")
        .args(&["lxc-info", "-P", LXC_PATH, "-n", name, "-s"])
        .output()
        .await;
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        return s.contains("RUNNING");
    }
    false
}

/// Tunggu kontainer sampai siap (network up).
async fn wait_for_ready(name: &str) {
    print!("{}[INFO]{} Menunggu kontainer siap", CYAN, RESET);
    for _ in 0..20 {
        if lxc_exec_silent(name, "true").await {
            println!(" {}OK{}", GREEN, RESET);
            return;
        }
        print!(".");
        sleep(Duration::from_secs(1)).await;
    }
    println!(" {}TIMEOUT{}", YELLOW, RESET);
}

/// Injeksi environment variables ke /etc/environment di dalam kontainer.
async fn inject_env_vars(container: &str, env: &std::collections::HashMap<String, String>) {
    for (key, val) in env {
        let line = format!("{}={}", key, val);
        // Hapus entry lama jika ada, lalu tambahkan
        let cmd = format!(
            "sed -i '/^{}=/d' /etc/environment && echo '{}' >> /etc/environment",
            key, line
        );
        let ok = lxc_exec_silent(container, &cmd).await;
        if ok {
            println!("{}[ENV]{} {} = {}", GREEN, RESET, key, val);
        } else {
            println!("{}[WARNING]{} Gagal set env: {}", YELLOW, RESET, key);
        }
    }
}

/// Jalankan daftar lifecycle hooks satu per satu.
async fn run_lifecycle_hooks(container: &str, hooks: &[String], phase: &str) {
    if hooks.is_empty() {
        println!("{}[INFO]{} Tidak ada hooks untuk phase '{}'.", YELLOW, RESET, phase);
        return;
    }
    println!("{}[LIFECYCLE]{} Fase: {}{}{}", CYAN, RESET, BOLD, phase, RESET);
    for (i, cmd) in hooks.iter().enumerate() {
        println!("  {}[{}/{}]{} $ {}", BOLD, i + 1, hooks.len(), RESET, cmd);
        let ok = lxc_exec(container, cmd).await;
        if !ok {
            println!("  {}[WARNING]{} Hook gagal: '{}'", YELLOW, RESET, cmd);
        }
    }
}

/// Jalankan health check dengan retry.
async fn run_health_check(
    container: &str,
    health: &crate::deployment::mel_parser::HealthSection,
) {
    let retries = health.retries.unwrap_or(3);
    let interval = health.interval.unwrap_or(5);

    for attempt in 1..=retries {
        println!("  {}[{}/{}]{} {}", BOLD, attempt, retries, RESET, health.command);
        if lxc_exec_silent(container, &health.command).await {
            println!("{}[OK]{} Health check berhasil!", GREEN, RESET);
            return;
        }
        if attempt < retries {
            println!("  {}[RETRY]{} Menunggu {}s...", YELLOW, RESET, interval);
            sleep(Duration::from_secs(interval as u64)).await;
        }
    }
    println!("{}[WARNING]{} Health check gagal setelah {} percobaan.", YELLOW, RESET, retries);
}

/// Cetak ringkasan manifest di awal deployment.
fn print_manifest_summary(manifest: &MelManifest, container_name: &str) {
    println!("\n  {}Proyek   :{} {} {}", BOLD, RESET,
        manifest.project.name,
        manifest.project.version.as_deref().map(|v| format!("v{}", v)).unwrap_or_default()
    );
    println!("  {}Kontainer:{} {}", BOLD, RESET, container_name);
    println!("  {}Distro   :{} {}", BOLD, RESET, manifest.container.distro);

    let dep_count = manifest.dependencies.apt.len()
        + manifest.dependencies.pip.len()
        + manifest.dependencies.npm.len()
        + manifest.dependencies.cargo.len();
    println!("  {}Deps     :{} {} paket total", BOLD, RESET, dep_count);
    println!("  {}Volumes  :{} {}", BOLD, RESET, manifest.volumes.mounts.len());
    println!("  {}Ports    :{} {}", BOLD, RESET, manifest.ports.expose.len());
}

/// Struct sederhana untuk rencana health check.
pub struct HealthCheckPlan {
    pub command:       String,
    pub retries:       u32,
    pub interval_secs: u64,
    pub timeout_secs:  u64,
}

/// Bangun rencana health check dari HealthSection manifest.
/// Semua field opsional punya nilai default yang wajar.
pub fn build_health_check_retry_plan(
    health: &crate::deployment::mel_parser::HealthSection,
) -> HealthCheckPlan {
    HealthCheckPlan {
        command:       health.command.clone(),
        retries:       health.retries.unwrap_or(3),
        interval_secs: health.interval.unwrap_or(5) as u64,
        timeout_secs:  health.timeout.unwrap_or(10) as u64,
    }
}

/// Bangun perintah shell untuk menyuntikkan satu env variable ke /etc/environment.
/// Menghapus entry lama dengan sed, lalu append yang baru.
pub fn build_env_inject_cmd(key: &str, value: &str) -> String {
    format!(
        "sed -i '/^{}=/d' /etc/environment && echo '{}={}' >> /etc/environment",
        key, key, value
    )
}

/// Format daftar port untuk ditampilkan di ringkasan.
pub fn format_ports_summary(ports: &[String]) -> String {
    if ports.is_empty() {
        return "(tidak ada)".to_string();
    }
    ports
        .iter()
        .map(|p| format!("  • {}", p))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format daftar volume mounts untuk ditampilkan di ringkasan.
pub fn format_volumes_summary(volumes: &[String]) -> String {
    if volumes.is_empty() {
        return "(tidak ada)".to_string();
    }
    volumes
        .iter()
        .map(|v| format!("  • {}", v))
        .collect::<Vec<_>>()
        .join("\n")
}