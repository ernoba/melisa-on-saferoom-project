use tokio::process::Command;
use std::process::Stdio;
use crate::cli::color_text::{GREEN, YELLOW, RED, BOLD, RESET};
use crate::core::container::LXC_PATH;
use crate::deployment::mel_parser::DependencySection;

/// Jalankan perintah shell di dalam kontainer via lxc-attach.
pub async fn lxc_exec(container: &str, shell_cmd: &str) -> bool {
    let status = Command::new("sudo")
        .args(&[
            "lxc-attach", "-P", LXC_PATH,
            "-n", container,
            "--", "sh", "-c", shell_cmd,
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await;
    status.map(|s| s.success()).unwrap_or(false)
}

/// Jalankan perintah di dalam kontainer, output disembunyikan (untuk cek).
pub async fn lxc_exec_silent(container: &str, shell_cmd: &str) -> bool {
    let status = Command::new("sudo")
        .args(&[
            "lxc-attach", "-P", LXC_PATH,
            "-n", container,
            "--", "sh", "-c", shell_cmd,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    status.map(|s| s.success()).unwrap_or(false)
}

/// Deteksi package manager yang tersedia di dalam kontainer.
pub async fn detect_pkg_manager(container: &str) -> Option<String> {
    for pm in &["apt-get", "pacman", "dnf", "apk", "zypper"] {
        if lxc_exec_silent(container, &format!("which {}", pm)).await {
            return Some(pm.to_string());
        }
    }
    None
}

/// Install paket sistem berdasarkan package manager distro kontainer.
pub async fn install_system_deps(
    container: &str,
    deps: &DependencySection,
    pkg_manager: &str,
) -> bool {
    let packages: &Vec<String> = match pkg_manager {
        "apt-get" | "apt" => &deps.apt,
        "pacman"          => &deps.pacman,
        "dnf" | "yum"     => &deps.dnf,
        "apk"             => &deps.apk,
        _ => {
            println!("{}[WARNING]{} Package manager '{}' tidak dikenali, skip system deps.", YELLOW, RESET, pkg_manager);
            return true;
        }
    };

    if packages.is_empty() {
        println!("{}[INFO]{} Tidak ada system dependency untuk '{}'.", YELLOW, RESET, pkg_manager);
        return true;
    }

    println!("{}[DEPLOY]{} Menginstall {} paket sistem via {}...{}", BOLD, RESET, packages.len(), pkg_manager, RESET);

    // Update repo terlebih dahulu
    let update_cmd = match pkg_manager {
        "pacman"       => "pacman -Sy --noconfirm",
        "apk"          => "apk update",
        "zypper"       => "zypper --non-interactive refresh",
        _              => "apt-get update -y",
    };
    let _ = lxc_exec_silent(container, update_cmd).await;

    // Build install command
    let pkg_list = packages.join(" ");
    let install_cmd = match pkg_manager {
        "pacman"       => format!("pacman -S --noconfirm {}", pkg_list),
        "apk"          => format!("apk add {}", pkg_list),
        "zypper"       => format!("zypper --non-interactive install {}", pkg_list),
        _              => format!("{} install -y {}", pkg_manager, pkg_list),
    };

    let ok = lxc_exec(container, &install_cmd).await;
    if ok {
        println!("{}[OK]{} System dependencies berhasil diinstall.", GREEN, RESET);
    } else {
        println!("{}[ERROR]{} Gagal install system dependencies.", RED, RESET);
    }
    ok
}

/// Install semua dependensi bahasa pemrograman (pip, npm, cargo, gem, composer).
pub async fn install_lang_deps(container: &str, deps: &DependencySection) -> bool {
    let mut all_ok = true;

    // ── pip ──────────────────────────────────────────────────────────────
    if !deps.pip.is_empty() {
        println!("{}[DEPLOY]{} Menginstall {} paket pip...{}", BOLD, RESET, deps.pip.len(), RESET);
        let pkgs = deps.pip.join(" ");
        let cmd = format!("pip3 install --break-system-packages {}", pkgs);
        let ok = lxc_exec(container, &cmd).await;
        if ok { println!("{}[OK]{} pip packages berhasil.", GREEN, RESET); }
        else  { println!("{}[ERROR]{} pip install gagal.", RED, RESET); all_ok = false; }
    }

    // ── npm (global) ──────────────────────────────────────────────────────
    if !deps.npm.is_empty() {
        println!("{}[DEPLOY]{} Menginstall {} paket npm (global)...{}", BOLD, RESET, deps.npm.len(), RESET);
        let pkgs = deps.npm.join(" ");
        let cmd = format!("npm install -g {}", pkgs);
        let ok = lxc_exec(container, &cmd).await;
        if ok { println!("{}[OK]{} npm packages berhasil.", GREEN, RESET); }
        else  { println!("{}[ERROR]{} npm install gagal.", RED, RESET); all_ok = false; }
    }

    // ── cargo install ─────────────────────────────────────────────────────
    for crate_name in &deps.cargo {
        println!("{}[DEPLOY]{} cargo install '{}'...{}", BOLD, RESET, crate_name, RESET);
        let cmd = format!("cargo install {}", crate_name);
        let ok = lxc_exec(container, &cmd).await;
        if !ok {
            println!("{}[ERROR]{} cargo install '{}' gagal.", RED, RESET, crate_name);
            all_ok = false;
        }
    }

    // ── gem ───────────────────────────────────────────────────────────────
    if !deps.gem.is_empty() {
        println!("{}[DEPLOY]{} Menginstall {} gem packages...{}", BOLD, RESET, deps.gem.len(), RESET);
        let pkgs = deps.gem.join(" ");
        let cmd = format!("gem install {}", pkgs);
        let ok = lxc_exec(container, &cmd).await;
        if ok { println!("{}[OK]{} gem packages berhasil.", GREEN, RESET); }
        else  { println!("{}[ERROR]{} gem install gagal.", RED, RESET); all_ok = false; }
    }

    // ── composer global require ───────────────────────────────────────────
    if !deps.composer.is_empty() {
        println!("{}[DEPLOY]{} Menginstall {} composer packages...{}", BOLD, RESET, deps.composer.len(), RESET);
        let pkgs = deps.composer.join(" ");
        let cmd = format!("composer global require {}", pkgs);
        let ok = lxc_exec(container, &cmd).await;
        if ok { println!("{}[OK]{} composer packages berhasil.", GREEN, RESET); }
        else  { println!("{}[ERROR]{} composer install gagal.", RED, RESET); all_ok = false; }
    }

    all_ok
}

/// Kembalikan perintah update repo sesuai package manager.
/// Fungsi ini sudah implisit ada di dalam `install_system_deps`,
/// kita expose untuk keperluan test.
pub fn build_update_cmd(pkg_manager: &str) -> String {
    match pkg_manager {
        "pacman"       => "pacman -Sy --noconfirm".to_string(),
        "apk"          => "apk update".to_string(),
        "zypper"       => "zypper --non-interactive refresh".to_string(),
        "dnf" | "yum"  => "dnf makecache".to_string(),
        _              => "apt-get update -y".to_string(),
    }
}

/// Bangun perintah install sistem berdasarkan package manager.
/// Return `None` jika tidak ada package yang perlu diinstall untuk PM tersebut.
pub fn build_system_install_cmd(pkg_manager: &str, deps: &DependencySection) -> Option<String> {
    let packages: &Vec<String> = match pkg_manager {
        "apt-get" | "apt" => &deps.apt,
        "pacman"          => &deps.pacman,
        "dnf" | "yum"     => &deps.dnf,
        "apk"             => &deps.apk,
        "zypper"          => &deps.zypper, // fallback ke apk jika pakai zypper (bisa disesuaikan)
        _                 => return None, // PM tidak dikenal
    };

    if packages.is_empty() {
        return None;
    }

    let pkg_list = packages.join(" ");
    let cmd = match pkg_manager {
        "pacman"      => format!("pacman -S --noconfirm {}", pkg_list),
        "apk"         => format!("apk add {}", pkg_list),
        "zypper"      => format!("zypper --non-interactive install {}", pkg_list),
        "dnf" | "yum" => format!("dnf install -y {}", pkg_list),
        _             => format!("apt-get install -y {}", pkg_list),
    };

    Some(cmd)
}

/// Periksa apakah ada dependensi bahasa pemrograman yang perlu diinstall.
pub fn has_lang_deps(deps: &DependencySection) -> bool {
    !deps.pip.is_empty()
        || !deps.npm.is_empty()
        || !deps.cargo.is_empty()
        || !deps.gem.is_empty()
        || !deps.composer.is_empty()
}