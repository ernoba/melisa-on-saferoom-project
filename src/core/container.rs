use tokio::process::Command;
use std::process::Stdio;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use std::path::Path;
use tokio::time::{sleep, Duration};
use std::path::PathBuf;

use crate::core::root_check::ensure_admin;
use crate::cli::color_text::{BOLD, GREEN, RED, RESET, YELLOW};

use crate::core::metadata::inject_distro_metadata;
use tracing::error;

use indicatif::ProgressBar;

use crate::distros::host_distro::{detect_host_distro, get_distro_config, FirewallKind};

pub const LXC_PATH: &str = "/var/lib/lxc";

#[derive(Debug, Clone)]
pub struct DistroMetadata {
    pub slug: String,
    pub name: String,
    pub release: String,
    pub arch: String,
    #[allow(dead_code)]
    pub variant: String,
    pub pkg_manager: String,
}

/// Creates a new LXC container using the download template.
/// Handles GPG errors, existing containers, and auto-initializes the network.
///
/// # Parameter `audit`
/// Ketika `true`:
///   - Semua `pb.println()` mengalir langsung ke terminal (ProgressBar::hidden).
///   - Output mentah dari `lxc-create` (stdout + stderr) diteruskan ke terminal
///     via `Stdio::inherit` sehingga pengguna melihat setiap baris yang biasanya
///     disembunyikan oleh `.output().await`.
pub async fn create_new_container(name: &str, meta: DistroMetadata, pb: ProgressBar, audit: bool) {
    // [STEP 0] PRE-FLIGHT: Verify host runtime environment (lxcbr0, etc.)
    if !verify_host_runtime(audit).await {
        eprintln!("{}[ERROR]{} Host network bridge is down and auto-repair failed.{}", RED, BOLD, RESET);
        eprintln!("{}Tip:{} Run 'melisa --setup' to initialize host infrastructure.", YELLOW, RESET);
        return;
    }

    pb.println(format!("{}--- Creating Container: {} ({}) ---{}", BOLD, name, meta.slug, RESET));

    if audit {
        // ── AUDIT MODE: tampilkan output mentah lxc-create ────────────────────
        pb.println(format!("{}[AUDIT]{} Running lxc-create — raw output follows:", YELLOW, RESET));

        let status = Command::new("sudo")
            .args(&[
                "-n", "lxc-create", "-P", LXC_PATH, "-t", "download", "-n", name,
                "--", "-d", &meta.name, "-r", &meta.release, "-a", &meta.arch,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(s) if s.success() => {
                pb.println(format!("{}[SUCCESS]{} Container successfully created.", GREEN, RESET));

                if let Err(e) = inject_distro_metadata(LXC_PATH, name, &meta).await {
                    error!("FATAL: Metadata injection failed: {}", e);
                }

                inject_network_config(name, &pb).await;
                setup_container_dns(name, &pb).await;

                pb.println(format!("{}[INFO]{} Starting container for initial setup...", YELLOW, RESET));
                start_container(name, audit).await;

                if wait_for_network_initialization(name, &pb).await {
                    auto_initial_setup(name, &meta.pkg_manager, &pb, audit).await;
                } else {
                    pb.println(format!(
                        "{}[ERROR]{} Network DHCP initialization timed out. Skipping package manager setup.",
                        RED, RESET
                    ));
                }

                pb.println(format!("{}[SUCCESS]{} Container successfully provisioned!", GREEN, RESET));
            }
            Ok(_) => {
                pb.println(format!("{}[ERROR]{} Failed to create container '{}'.", RED, RESET, name));
            }
            Err(e) => {
                eprintln!("{}[FATAL]{} Could not execute lxc-create command: {}", RED, RESET, e);
            }
        }
    } else {
        // ── NORMAL MODE: telan output subprocess, tampilkan hanya ringkasan ───
        let process = Command::new("sudo")
            .args(&[
                "-n", "lxc-create", "-P", LXC_PATH, "-t", "download", "-n", name,
                "--", "-d", &meta.name, "-r", &meta.release, "-a", &meta.arch,
            ])
            .output()
            .await;

        match process {
            Ok(output) => {
                if output.status.success() {
                    pb.println(format!("{}[SUCCESS]{} Container successfully created.", GREEN, RESET));

                    if let Err(e) = inject_distro_metadata(LXC_PATH, name, &meta).await {
                        error!("FATAL: Metadata injection failed: {}", e);
                    }

                    inject_network_config(name, &pb).await;
                    setup_container_dns(name, &pb).await;

                    pb.println(format!("{}[INFO]{} Starting container for initial setup...", YELLOW, RESET));
                    start_container(name, audit).await;

                    if wait_for_network_initialization(name, &pb).await {
                        auto_initial_setup(name, &meta.pkg_manager, &pb, audit).await;
                    } else {
                        pb.println(format!(
                            "{}[ERROR]{} Network DHCP initialization timed out. Skipping package manager setup.",
                            RED, RESET
                        ));
                    }

                    pb.println(format!("{}[SUCCESS]{} Container successfully provisioned!", GREEN, RESET));
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);

                    if error_msg.contains("already exists") {
                        pb.println(format!(
                            "{}[WARNING]{} Container '{}' already exists. Skipping creation process.",
                            YELLOW, RESET, name
                        ));
                    } else if error_msg.contains("GPG") {
                        pb.println(format!(
                            "{}[ERROR]{} GPG signature verification failed. Try running 'gpg --recv-keys' on the host system.",
                            RED, RESET
                        ));
                    } else if error_msg.contains("download") {
                        pb.println(format!(
                            "{}[ERROR]{} Failed to download template. Please verify the host's internet connection.",
                            RED, RESET
                        ));
                    } else {
                        pb.println(format!("{}[ERROR]{} Failed to create container: {}", RED, RESET, name));
                        pb.println(format!("Error Details: {}", error_msg));
                    }
                }
            }
            Err(e) => eprintln!("{}[FATAL]{} Could not execute lxc-create command: {}", RED, RESET, e),
        }
    }
}

/// Performs a lightweight pre-flight check on the host system's networking.
/// If the required bridge is missing, it attempts an automatic repair.
async fn verify_host_runtime(audit: bool) -> bool {
    if Path::new("/sys/class/net/lxcbr0").exists() {
        return true;
    }

    println!("{}[WARNING]{} Network bridge 'lxcbr0' not found. Initiating host auto-repair...", YELLOW, RESET);

    ensure_host_network_ready(audit).await;

    Path::new("/sys/class/net/lxcbr0").exists()
}

/// Dynamically polls LXC to check if the container has successfully acquired an IP address.
/// Semua println! di fungsi ini sekarang menggunakan pb.println() agar tidak
/// konflik dengan spinner yang sedang berjalan di luar.
async fn wait_for_network_initialization(name: &str, pb: &ProgressBar) -> bool {
    pb.println(format!(
        "{}[INFO]{} Waiting for DHCP lease and network interfaces to initialize...",
        YELLOW, RESET
    ));

    let max_retries = 30;

    for _ in 0..max_retries {
        let output = Command::new("sudo")
            .args(&["-n", "lxc-info", "-n", name, "-iH"])
            .output()
            .await;

        if let Ok(out) = output {
            let ips = String::from_utf8_lossy(&out.stdout);

            if ips.contains('.') && !ips.trim().is_empty() {
                pb.println(format!(
                    "{}[INFO]{} Network connection established. Allowing DNS resolver to settle...",
                    YELLOW, RESET
                ));
                sleep(Duration::from_secs(3)).await;
                return true;
            }
        }

        sleep(Duration::from_secs(1)).await;
    }

    false
}

/// Automatically updates the package repository of the newly created container
/// based on its specific package manager (apt, dnf, apk, etc.)
///
/// Ketika `audit = true`, stdout dan stderr dari package manager diteruskan
/// langsung ke terminal sehingga pengguna melihat seluruh output instalasi.
async fn auto_initial_setup(name: &str, pkg_manager: &str, pb: &ProgressBar, audit: bool) {
    let cmd = match pkg_manager {
        "apt"    => "apt-get update -y",
        "dnf"    => "dnf makecache",
        "apk"    => "apk update",
        "pacman" => "pacman -Sy --noconfirm",
        "zypper" => "zypper --non-interactive refresh",
        _        => "true",
    };

    pb.println(format!("{}[INFO]{} Updating package repository for '{}'...", YELLOW, RESET, name));

    if audit {
        // ── AUDIT MODE: teruskan output package manager ke terminal ────────────
        pb.println(format!("{}[AUDIT]{} Running '{}' — raw output follows:", YELLOW, RESET, cmd));

        let status = Command::new("sudo")
            .args(&["-n", "lxc-attach", "-n", name, "--", "sh", "-c", cmd])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(s) if s.success() => {
                pb.println(format!("{}[SUCCESS]{} Initial repository setup completed for {}.", GREEN, RESET, name));
            }
            Ok(_) => {
                pb.println(format!(
                    "{}[ERROR]{} Failed to execute initial repository setup on {}.",
                    RED, RESET, name
                ));
            }
            Err(e) => {
                eprintln!("{}[FATAL]{} Failed to spawn lxc-attach process: {}", RED, RESET, e);
            }
        }
    } else {
        // ── NORMAL MODE: telan output, tampilkan hanya status akhir ───────────
        let output = Command::new("sudo")
            .args(&["-n", "lxc-attach", "-n", name, "--", "sh", "-c", cmd])
            .output()
            .await;

        match output {
            Ok(out) => {
                if out.status.success() {
                    pb.println(format!("{}[SUCCESS]{} Initial repository setup completed for {}.", GREEN, RESET, name));
                } else {
                    pb.println(format!(
                        "{}[ERROR]{} Failed to execute initial repository setup on {}.",
                        RED, RESET, name
                    ));
                    // Hanya di mode normal kita cetak debug stderr — di audit mode
                    // stderr sudah langsung tampil via Stdio::inherit di atas.
                    eprintln!("[DEBUG] Package Manager Error: {}", String::from_utf8_lossy(&out.stderr));
                }
            }
            Err(e) => eprintln!("{}[FATAL]{} Failed to spawn lxc-attach process: {}", RED, RESET, e),
        }
    }
}

#[allow(dead_code)]
pub fn get_pkg_update_cmd(pkg_manager: &str) -> &'static str {
    match pkg_manager {
        "apt"    => "apt-get update -y",
        "dnf"    => "dnf makecache",
        "apk"    => "apk update",
        "pacman" => "pacman -Sy --noconfirm",
        "zypper" => "zypper --non-interactive refresh",
        _        => "true",
    }
}

/// Injects a veth bridge network configuration into the container's config file.
/// Ensures the container is connected to lxcbr0 with a random MAC address.
/// pb diterima sebagai parameter agar println! tidak konflik dengan spinner.
async fn inject_network_config(name: &str, pb: &ProgressBar) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);

    if Path::new(&config_path).exists() {
        let content = fs::read_to_string(&config_path).await.unwrap_or_default();

        if content.contains("lxc.net.0.link") {
            pb.println(format!(
                "{}[SKIP]{} Network configuration already exists. Skipping injection.",
                YELLOW, RESET
            ));
            return;
        }

        match OpenOptions::new().append(true).open(&config_path).await {
            Ok(mut file) => {
                let net_config = format!(
                    "\n# Auto-generated by MELISA\n\
                    lxc.net.0.type = veth\n\
                    lxc.net.0.link = lxcbr0\n\
                    lxc.net.0.flags = up\n\
                    lxc.net.0.hwaddr = ee:ec:fa:5e:{:02x}:{:02x}\n",
                    rand::random::<u8>(),
                    rand::random::<u8>()
                );

                if let Err(e) = file.write_all(net_config.as_bytes()).await {
                    eprintln!("{}[ERROR]{} Failed to write async network config: {}", RED, RESET, e);
                }
            }
            Err(e) => eprintln!(
                "{}[ERROR]{} Failed to open container configuration file asynchronously: {}",
                RED, RESET, e
            ),
        }
    }
}

/// Injects a static DNS configuration (Google DNS) into the container's rootfs
/// and applies an immutable lock to prevent overwrites by network managers.
/// pb diterima sebagai parameter agar println! tidak konflik dengan spinner.
async fn setup_container_dns(name: &str, pb: &ProgressBar) {
    let etc_path = format!("{}/{}/rootfs/etc", LXC_PATH, name);
    let dns_path = format!("{}/resolv.conf", etc_path);

    let _ = Command::new("sudo")
        .args(&["mkdir", "-p", &etc_path])
        .status()
        .await;

    let _ = Command::new("sudo")
        .args(&["rm", "-f", &dns_path])
        .status()
        .await;

    let dns_content = "nameserver 8.8.8.8\\nnameserver 8.8.4.4\\n";
    let write_status = Command::new("sudo")
        .args(&["bash", "-c", &format!("echo -e '{}' > {}", dns_content, dns_path)])
        .status()
        .await;

    match write_status {
        Ok(s) if s.success() => {
            let lock_status = Command::new("sudo")
                .args(&["chattr", "+i", &dns_path])
                .status()
                .await;

            if let Ok(ls) = lock_status {
                if ls.success() {
                    pb.println(format!("{}[INFO]{} DNS configured and locked successfully.", GREEN, RESET));
                } else {
                    pb.println(format!(
                        "{}[WARNING]{} DNS written, but failed to apply immutable lock (chattr).",
                        YELLOW, RESET
                    ));
                }
            }
        }
        _ => eprintln!("{}[ERROR]{} Failed to configure DNS.", RED, RESET),
    }
}

/// Helper function to unlock the DNS file later if needed.
#[allow(dead_code)]
async fn unlock_container_dns(name: &str) {
    let dns_path = format!("{}/{}/rootfs/etc/resolv.conf", LXC_PATH, name);
    let _ = Command::new("sudo")
        .args(&["-n", "chattr", "-i", &dns_path])
        .status()
        .await;
}

/// Ensures the host system's LXC bridge network and firewall are active.
/// Upgraded to dynamically detect and configure the host's firewall rules.
///
/// Ketika `audit = true`, semua output systemctl dan firewall diteruskan ke terminal.
pub async fn ensure_host_network_ready(audit: bool) {
    println!("{}[INFO]{} Re-initializing Host Network Infrastructure...", BOLD, RESET);

    if audit {
        let _ = Command::new("sudo")
            .args(&["-n", "systemctl", "start", "lxc-net"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;
    } else {
        let _ = Command::new("sudo")
            .args(&["-n", "systemctl", "start", "lxc-net"])
            .status()
            .await;
    }

    let distro = detect_host_distro().await;
    let cfg = get_distro_config(&distro);

    match cfg.firewall_tool {
        FirewallKind::Firewalld => {
            let _ = Command::new("sudo")
                .args(&["-n", "firewall-cmd", "--zone=trusted", "--add-interface=lxcbr0", "--permanent"])
                .status()
                .await;
            let _ = Command::new("sudo")
                .args(&["-n", "firewall-cmd", "--reload"])
                .status()
                .await;
        }
        FirewallKind::Ufw => {
            let _ = Command::new("sudo")
                .args(&["-n", "ufw", "allow", "in", "on", "lxcbr0"])
                .status()
                .await;
            let _ = Command::new("sudo")
                .args(&["-n", "ufw", "reload"])
                .status()
                .await;
        }
        FirewallKind::Iptables => {
            let _ = Command::new("sudo")
                .args(&["-n", "iptables", "-I", "INPUT", "-i", "lxcbr0", "-j", "ACCEPT"])
                .status()
                .await;
        }
    }
}

/// Helper function to check if a specific container is currently running.
async fn is_container_running(name: &str) -> bool {
    let output = Command::new("sudo")
        .args(&["-n", "lxc-info", "-P", LXC_PATH, "-n", name, "-s"])
        .output()
        .await;

    match output {
        Ok(out) => {
            let status_str = String::from_utf8_lossy(&out.stdout);
            status_str.contains("RUNNING")
        }
        _ => false,
    }
}

/// Menghapus file metadata MELISA (info & tmp) secara eksplisit.
async fn cleanup_metadata(name: &str) {
    let rootfs_path = PathBuf::from(LXC_PATH).join(name).join("rootfs");
    let target_path = rootfs_path.join("etc").join("melisa-info");
    let temp_path = rootfs_path.join("etc").join("melisa-info.tmp");

    if tokio::fs::try_exists(&target_path).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(&target_path).await;
    }

    if tokio::fs::try_exists(&temp_path).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
}

/// Gracefully stops and destroys a container.
/// Automatically handles running containers and unlocks restricted files before deletion.
///
/// Ketika `audit = true`:
///   - Semua log detail muncul di terminal.
///   - Output dari lxc-destroy diteruskan langsung (Stdio::inherit).
pub async fn delete_container(name: &str, pb: ProgressBar, audit: bool) {
    pb.println(format!("{}--- Processing Deletion: {} ---{}", BOLD, name, RESET));

    if is_container_running(name).await {
        pb.println(format!("{}[INFO]{} Container '{}' is currently running.", YELLOW, RESET, name));
        pb.println(format!("{}[INFO]{} Initiating graceful shutdown before deletion...", YELLOW, RESET));

        stop_container(name, audit).await;

        if is_container_running(name).await {
            eprintln!(
                "{}[ERROR]{} Failed to stop container '{}'. Deletion aborted to prevent data corruption.",
                RED, RESET, name
            );
            return;
        }
    }

    pb.println(format!("{}[INFO]{} Unlocking system configurations for {}...", BOLD, RESET, name));
    unlock_container_dns(name).await;

    pb.println(format!("{}[INFO]{} Purging MELISA engine metadata for {}...", BOLD, RESET, name));
    cleanup_metadata(name).await;

    if audit {
        // ── AUDIT MODE: tampilkan output mentah lxc-destroy ───────────────────
        pb.println(format!("{}[AUDIT]{} Running lxc-destroy — raw output follows:", YELLOW, RESET));

        let status = Command::new("sudo")
            .args(&["-n", "lxc-destroy", "-P", LXC_PATH, "-n", name, "-f"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(s) if s.success() => {
                pb.println(format!(
                    "{}[SUCCESS]{} Container '{}' has been permanently destroyed.",
                    GREEN, RESET, name
                ));
            }
            Ok(s) => {
                eprintln!(
                    "{}[ERROR]{} Deletion failed with exit code: {}.",
                    RED, RESET,
                    s.code().unwrap_or(-1)
                );
            }
            Err(e) => eprintln!("{}[FATAL]{} Could not execute lxc-destroy: {}", RED, RESET, e),
        }
    } else {
        // ── NORMAL MODE ────────────────────────────────────────────────────────
        let status = Command::new("sudo")
            .args(&["-n", "lxc-destroy", "-P", LXC_PATH, "-n", name, "-f"])
            .status()
            .await;

        match status {
            Ok(s) if s.success() => {
                pb.println(format!(
                    "{}[SUCCESS]{} Container '{}' has been permanently destroyed.",
                    GREEN, RESET, name
                ));
            }
            Ok(s) => {
                eprintln!(
                    "{}[ERROR]{} Deletion failed with exit code: {}.",
                    RED, RESET,
                    s.code().unwrap_or(-1)
                );
                eprintln!(
                    "{}[TIP]{} Ensure you have sudo permissions or check 'lxc-ls' for container status.",
                    YELLOW, RESET
                );
            }
            Err(e) => eprintln!("{}[FATAL]{} Could not execute lxc-destroy: {}", RED, RESET, e),
        }
    }
}

/// Boots up a container in daemon (-d) mode.
///
/// Ketika `audit = true`, output lxc-start diteruskan ke terminal.
pub async fn start_container(name: &str, audit: bool) {
    println!("{}[INFO]{} Starting container '{}'...", GREEN, RESET, name);

    if audit {
        let status = Command::new("sudo")
            .args(&["lxc-start", "-P", LXC_PATH, "-n", name, "-d"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(s) if s.success() => println!("{}[SUCCESS]{} Container is now running.", GREEN, RESET),
            _ => eprintln!(
                "{}[ERROR]{} Failed to start container. Check if it exists and is configured properly.",
                RED, RESET
            ),
        }
    } else {
        let status = Command::new("sudo")
            .args(&["lxc-start", "-P", LXC_PATH, "-n", name, "-d"])
            .status()
            .await;

        match status {
            Ok(s) if s.success() => println!("{}[SUCCESS]{} Container is now running.", GREEN, RESET),
            _ => eprintln!(
                "{}[ERROR]{} Failed to start container. Check if it exists and is configured properly.",
                RED, RESET
            ),
        }
    }
}

/// Attaches the host terminal directly into the container's bash session.
pub async fn attach_to_container(name: &str) {
    println!("{}[MODE]{} Entering Saferoom: {}. Type 'exit' to return to Host.", BOLD, name, RESET);

    let _ = Command::new("sudo")
        .args(&["lxc-attach", "-P", LXC_PATH, "-n", name, "--", "bash"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .status()
        .await;
}

/// Gracefully powers down a running container.
///
/// Ketika `audit = true`, output lxc-stop diteruskan ke terminal.
pub async fn stop_container(name: &str, audit: bool) {
    if !ensure_admin().await {
        return;
    }
    println!("{}[SHUTDOWN]{} Initiating shutdown for container '{}'...", YELLOW, RESET, name);

    if audit {
        let status = Command::new("sudo")
            .args(&["lxc-stop", "-P", LXC_PATH, "-n", name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(s) if s.success() => {
                println!("{}[SUCCESS]{} Container '{}' has been successfully stopped.", GREEN, RESET, name);
            }
            Ok(_) => eprintln!("{}[ERROR]{} Failed to stop container.", RED, RESET),
            Err(e) => eprintln!("{}[FATAL]{} Execution Error: {}", RED, RESET, e),
        }
    } else {
        let process = Command::new("sudo")
            .args(&["lxc-stop", "-P", LXC_PATH, "-n", name])
            .output()
            .await;

        match process {
            Ok(output) => {
                if output.status.success() {
                    println!("{}[SUCCESS]{} Container '{}' has been successfully stopped.", GREEN, RESET, name);
                } else {
                    eprintln!("{}[ERROR]{} Failed to stop container.", RED, RESET);
                }
            }
            Err(e) => eprintln!("{}[FATAL]{} Execution Error: {}", RED, RESET, e),
        }
    }
}

/// Sends a direct execution command to a running container from the host.
pub async fn send_command(name: &str, command_args: &[&str]) {
    if command_args.is_empty() {
        eprintln!("{}[ERROR]{} No command payload provided.", RED, RESET);
        return;
    }

    let check_status = Command::new("sudo")
        .args(&["/usr/bin/lxc-info", "-P", LXC_PATH, "-n", name, "-s"])
        .output()
        .await;

    if let Ok(out) = check_status {
        let output_str = String::from_utf8_lossy(&out.stdout);
        if !output_str.contains("RUNNING") {
            println!("{}[ERROR]{} Container '{}' is NOT running.", RED, RESET, name);
            println!("{}Tip:{} Execute 'melisa --run {}' to start it first.", YELLOW, RESET, name);
            return;
        }
    } else {
        eprintln!("{}[ERROR]{} Failed to retrieve container status.", RED, RESET);
        return;
    }

    println!("{}[SEND]{} Executing payload on '{}'...", BOLD, name, RESET);

    let status = Command::new("sudo")
        .arg("lxc-attach")
        .arg("-P")
        .arg(LXC_PATH)
        .arg("-n")
        .arg(name)
        .arg("--")
        .args(command_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            println!("\n{}[DONE]{} Command executed successfully within container.", GREEN, RESET)
        }
        _ => eprintln!(
            "\n{}[ERROR]{} Command inside container returned a non-zero exit code.",
            RED, RESET
        ),
    }
}

/// Mounts a directory from the Host system to the Container via bind mount.
pub async fn add_shared_folder(name: &str, host_path: &str, container_path: &str) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);

    if let Err(e) = fs::create_dir_all(host_path).await {
    eprintln!("{}[ERROR]{} Gagal membuat direktori host '{}': {}", RED, RESET, host_path, e);
    return;
}
    // Baru canonicalize — sekarang pasti exist
    let abs_host_path = match fs::canonicalize(host_path).await {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}[ERROR]{} Gagal resolve path '{}': {}", RED, RESET, host_path, e);
            return;
        }
    };

    if Path::new(&config_path).exists() {
        let content = fs::read_to_string(&config_path).await.unwrap_or_default();
        let mount_entry = format!("lxc.mount.entry = {} {}", abs_host_path.display(), container_path);

        if content.contains(&mount_entry) {
            println!("{}[SKIP]{} This directory is already mapped in the configuration.", YELLOW, RESET);
            return;
        }

        match OpenOptions::new().append(true).open(&config_path).await {
            Ok(mut file) => {
                let mount_config = format!(
                    "\n# Shared Folder mapped by MELISA\n\
                    lxc.mount.entry = {} {} none bind,create=dir 0 0\n",
                    abs_host_path.display(),
                    container_path
                );

                match file.write_all(mount_config.as_bytes()).await {
                    Ok(_) => {
                        println!("{}[SUCCESS]{} Shared folder integrated to {}.", GREEN, RESET, name);
                        println!(
                            "{}[IMPORTANT]{} Please run 'melisa --stop {}' and 'melisa --run {}' to apply changes.",
                            YELLOW, RESET, name, name
                        );
                    }
                    Err(e) => eprintln!("{}[ERROR]{} Failed to write mount configuration: {}", RED, RESET, e),
                }
            }
            Err(e) => eprintln!("{}[ERROR]{} Failed to open container configuration: {}", RED, RESET, e),
        }
    } else {
        eprintln!("{}[ERROR]{} Configuration file for container '{}' not found.", RED, RESET, name);
    }
}

/// Removes a previously mounted shared folder from the container's configuration.
pub async fn remove_shared_folder(name: &str, host_path: &str, container_path: &str) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);

    let abs_host_path = match fs::canonicalize(host_path).await {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}[ERROR]{} Host path not found or invalid: {}", RED, RESET, e);
            return;
        }
    };
    let host_path_str = abs_host_path.to_string_lossy();

    if Path::new(&config_path).exists() {
        let content = match fs::read_to_string(&config_path).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}[ERROR]{} Failed to read container configuration: {}", RED, RESET, e);
                return;
            }
        };

        let target_entry = format!("lxc.mount.entry = {} {}", host_path_str, container_path);
        let comment_tag = "# Shared Folder mapped by MELISA";

        let lines: Vec<&str> = content.lines().collect();
        let mut new_lines = Vec::new();
        let mut removed = false;

        let mut i = 0;
        while i < lines.len() {
            if lines[i].contains(&target_entry) {
                if !new_lines.is_empty() && new_lines.last() == Some(&comment_tag) {
                    new_lines.pop();
                }
                removed = true;
                i += 1;
                continue;
            }
            new_lines.push(lines[i]);
            i += 1;
        }

        if !removed {
            println!(
                "{}[SKIP]{} Shared folder mapping was not found in the configuration.",
                YELLOW, RESET
            );
            return;
        }

        let new_content = new_lines.join("\n");
        match fs::write(&config_path, new_content).await {
            Ok(_) => {
                println!("{}[SUCCESS]{} Shared folder successfully unmapped from {}.", GREEN, RESET, name);
                println!("{}[IMPORTANT]{} Please restart the container to apply changes.", YELLOW, RESET);
            }
            Err(e) => eprintln!("{}[ERROR]{} Failed to update configuration file: {}", RED, RESET, e),
        }
    } else {
        eprintln!("{}[ERROR]{} Container configuration file not found.", RED, RESET);
    }
}

/// Securely pipes a tarball from standard input directly into the container's filesystem.
pub async fn upload_to_container(name: &str, dest_path: &str) {
    let extract_cmd = format!("mkdir -p {} && tar -xzf - -C {}", dest_path, dest_path);

    let status = Command::new("sudo")
        .args(&["lxc-attach", "-P", LXC_PATH, "-n", name, "--", "bash", "-c", &extract_cmd])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await;

    match status {
        Ok(s) if s.success() => println!(
            "{}[SUCCESS]{} Upload and extraction to '{}' completed successfully.",
            GREEN, RESET, dest_path
        ),
        _ => eprintln!(
            "{}[ERROR]{} Failed to extract data stream inside the container.",
            RED, RESET
        ),
    }
}

/// Displays a list of existing containers using lxc-ls.
pub async fn list_containers(only_active: bool) {
    println!("{}[INFO]{} Retrieving container inventory...", GREEN, RESET);

    let mut cmd = Command::new("sudo");
    cmd.args(&["lxc-ls", "-P", LXC_PATH, "--fancy"]);

    if only_active {
        cmd.arg("--active");
    }

    let output = cmd.output().await;

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("{}", String::from_utf8_lossy(&out.stdout));
            } else {
                eprintln!("{}[ERROR]{} Failed to retrieve container list.", RED, RESET);
            }
        }
        Err(e) => eprintln!("{}[FATAL]{} System Error: {}", RED, RESET, e),
    }
}

/// Ambil IP internal container LXC (untuk keperluan SSH tunnel)
pub async fn get_container_ip(name: &str) -> Option<String> {
    let output = Command::new("sudo")
        .args(&["lxc-info", "-P", LXC_PATH, "-n", name, "-i"])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // lxc-info -i output: "IP:  10.0.3.5\n"
            for line in stdout.lines() {
                let line = line.trim();
                if line.starts_with("IP:") {
                    let ip = line.trim_start_matches("IP:").trim().to_string();
                    if !ip.is_empty() && !ip.contains("127.") {
                        return Some(ip);
                    }
                }
            }
            None
        }
        _ => None,
    }
}