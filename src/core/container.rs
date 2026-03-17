use std::process::{Command, Stdio};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

// Fungsi ini sekarang menerima DistroMetadata
use std::thread;
use std::time::Duration;

use crate::core::root_check::ensure_admin;
use crate::cli::color_text::{BOLD, GREEN, RED, RESET, YELLOW}; 

pub const LXC_PATH: &str = "/var/lib/lxc"; // Tambah pub

#[derive(Debug, Clone)]
pub struct DistroMetadata {
    pub slug: String,       
    pub name: String,       
    pub release: String,    
    pub arch: String,       
    #[allow(dead_code)]
    pub variant: String,    // Pastikan ini ada
    pub pkg_manager: String 
}

pub fn create_new_container(name: &str, meta: DistroMetadata) {
    if !ensure_admin() { return; }
    ensure_host_network_ready();

    println!("{}--- Creating Container: {} ({}) ---{}", BOLD, name, meta.slug, RESET);
    
    let process = Command::new("sudo")
        .args(&[
            "lxc-create", "-P", LXC_PATH, "-t", "download", "-n", name, 
            "--", "-d", &meta.name, "-r", &meta.release, "-a", &meta.arch
        ])
        .output();

    match process {
        Ok(output) => {
            if output.status.success() {
                println!("{}[SUCCESS]{} Container created.", GREEN, RESET);
                
                // 1. Injeksi konfigurasi jaringan & DNS saat kontainer masih mati
                inject_network_config(name);
                setup_container_dns(name); 

                // 2. PERBAIKAN: Nyalakan kontainer terlebih dahulu!
                println!("{}[INFO]{} Starting container for initial setup...", YELLOW, RESET);
                start_container(name);

                // 3. PERBAIKAN: Tunggu beberapa detik agar DHCP mendapatkan IP Address
                println!("{}[INFO]{} Menunggu antarmuka jaringan dan DHCP siap (5 detik)...", YELLOW, RESET);
                thread::sleep(Duration::from_secs(5));

                // 4. Setelah jaringan siap, baru eksekusi setup package manager
                auto_initial_setup(name, &meta.pkg_manager);
                
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                
                if error_msg.contains("already exists") {
                    println!("{}[WARNING]{} Container '{}' sudah ada. Melewati proses pembuatan.", YELLOW, RESET, name);
                } else if error_msg.contains("GPG") {
                    eprintln!("{}[ERROR]{} Masalah tanda tangan GPG. Coba jalankan 'gpg --recv-keys' di host.", RED, RESET);
                } else if error_msg.contains("download") {
                    eprintln!("{}[ERROR]{} Gagal mendownload template. Pastikan koneksi internet host aktif.", RED, RESET);
                } else {
                    eprintln!("{}[ERROR]{} Gagal membuat container: {}", RED, RESET, name);
                    eprintln!("Detail Error: {}", error_msg);
                }
            }
        },
        Err(e) => eprintln!("{}[FATAL]{} Could not run lxc-create: {}", RED, RESET, e),
    }
}

fn auto_initial_setup(name: &str, pkg_manager: &str) {
    let cmd = match pkg_manager {
        "apt"    => "apt-get update -y",                           // Ubuntu/Debian
        "dnf"    => "dnf makecache",                               // Fedora/RHEL baru
        "yum"    => "yum makecache",                               // CentOS/RHEL lama
        "apk"    => "apk update",                                  // Alpine
        "pacman" => "pacman -Sy --noconfirm",                      // Arch Linux
        "zypper" => "zypper --non-interactive refresh",            // openSUSE/SLES
        _        => "true",                                        // Fallback aman jika tidak dikenali
    };
    
    println!("{}[INFO]{} Updating package repository for '{}'...", YELLOW, RESET, name);

    // Tetap menggunakan "sh" agar kompatibel dengan distro minimalis seperti Alpine
    let status = Command::new("sudo")
        .args(&["lxc-attach", "-n", name, "--", "sh", "-c", cmd])
        .status();

    match status {
        Ok(s) if s.success() => println!("{}[SUCCESS]{} Initial setup (repo update) completed for {}.", GREEN, RESET, name),
        _ => eprintln!("{}[ERROR]{} Failed to run initial setup on {}.", RED, RESET, name),
    }
}

//fungsi memastikan internet container bekerja 
fn inject_network_config(name: &str) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);
    
    if Path::new(&config_path).exists() {
        // Baca dulu isinya
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        
        // Jika sudah ada lxc.net.0.link, jangan disuntik lagi!
        if content.contains("lxc.net.0.link") {
            println!("{}[SKIP]{} Network configuration already exists.", YELLOW, RESET);
            return;
        }

        let mut file = OpenOptions::new()
            .append(true)
            .open(&config_path)
            .expect("Gagal membuka config container");

        let net_config = format!(
            "\n# Auto-generated by MELISA\n\
            lxc.net.0.type = veth\n\
            lxc.net.0.link = lxcbr0\n\
            lxc.net.0.flags = up\n\
            lxc.net.0.hwaddr = ee:ec:fa:5e:{:02x}:{:02x}\n", 
            rand::random::<u8>(), rand::random::<u8>()
        );

        file.write_all(net_config.as_bytes()).ok();
    }
}
fn setup_container_dns(name: &str) {
    // Path menuju resolv.conf di dalam rootfs container
    let dns_path = format!("{}/{}/rootfs/etc/resolv.conf", LXC_PATH, name);
    
    let dns_content = "nameserver 8.8.8.8\nnameserver 8.8.4.4\n";
    
    match std::fs::write(&dns_path, dns_content) {
        Ok(_) => println!("{}[INFO]{} DNS configured (Google DNS).", GREEN, RESET),
        Err(e) => eprintln!("{}[ERROR]{} Gagal set DNS: {}", RED, RESET, e),
    }
}

pub fn ensure_host_network_ready() {
    // Pastikan lxc-net aktif
    let _ = Command::new("systemctl")
        .args(&["start", "lxc-net"])
        .status();

    // Pastikan firewalld mengizinkan lxcbr0
    let _ = Command::new("firewall-cmd")
        .args(&["--zone=trusted", "--add-interface=lxcbr0", "--permanent"])
        .status();
    
    let _ = Command::new("firewall-cmd")
        .args(&["--reload"])
        .status();
}

pub fn delete_container(name: &str) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Deleting Container: {} ---{}", BOLD, name, RESET);

    let process = Command::new("sudo")
        .args(&["lxc-destroy", "-P", LXC_PATH, "-f", "-n", name])
        .output();

    match process {
        Ok(output) => {
            if output.status.success() {
                println!("{}[SUCCESS]{} Container '{}' deleted.", GREEN, RESET, name);
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                eprintln!("{}[ERROR]{} Failed to delete container: {}", RED, RESET, error_msg);
            }
        },
        Err(e) => eprintln!("{}[FATAL]{} Could not run lxc-destroy: {}", RED, RESET, e),
    }
}

pub fn start_container(name: &str) {
    println!("{}[INFO]{} Starting container '{}'...", GREEN, RESET, name);
    
    let status = Command::new("sudo")
        .args(&["lxc-start", "-P", LXC_PATH, "-n", name, "-d"]) 
        .status();

    match status {
        Ok(s) if s.success() => println!("{}[SUCCESS]{} Container is now running.", GREEN, RESET),
        _ => eprintln!("{}[ERROR]{} Failed to start container. Check if it exists.", RED, RESET),
    }
}

pub fn attach_to_container(name: &str) {
    println!("{}[MODE]{} Entering Saferoom: {}. Type 'exit' to return.", BOLD, name, RESET);

    let _ = Command::new("sudo")
        .args(&["lxc-attach", "-P", LXC_PATH, "-n", name, "--", "bash"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .status();
}

pub fn stop_container(name: &str) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}[SHUTDOWN]{} Stopping container '{}'...", YELLOW, RESET, name);

    let process = Command::new("sudo")
        .args(&["lxc-stop", "-P", LXC_PATH, "-n", name])
        .output();

    match process {
        Ok(output) => {
            if output.status.success() {
                println!("{}[SUCCESS]{} Container '{}' stopped.", GREEN, RESET, name);
            } else {
                eprintln!("{}[ERROR]{} Failed to stop container.", RED, RESET);
            }
        },
        Err(e) => eprintln!("{}[FATAL]{} Error: {}", RED, RESET, e),
    }
}

pub fn send_command(name: &str, command_args: &[&str]) {
    if command_args.is_empty() {
        eprintln!("{}[ERROR]{} No command provided.", RED, RESET);
        return;
    }

    // 1. CEK STATUS DULU (Pre-flight Check)
    let check_status = Command::new("sudo")
        .args(&["/usr/bin/lxc-info", "-P", LXC_PATH, "-n", name, "-s"])
        .output();

    if let Ok(out) = check_status {
        let output_str = String::from_utf8_lossy(&out.stdout);
        if !output_str.contains("RUNNING") {
            println!("{}[ERROR]{} Container '{}' is NOT running.", RED, RESET, name);
            println!("{}Tip:{} Run 'melisa --run {}' first.", YELLOW, RESET, name);
            return; // Berhenti di sini, jangan lanjut eksekusi
        }
    } else {
        eprintln!("{}[ERROR]{} Gagal mengecek status kontainer.", RED, RESET);
        return;
    }

    // 2. JIKA RUNNING, BARU EKSEKUSI
    println!("{}[SEND]{} Executing on '{}'...", BOLD, name, RESET);

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
        .status();

    // 3. CEK APAKAH PERINTAHNYA BERHASIL
    match status {
        Ok(s) if s.success() => println!("\n{}[DONE]{} Command executed successfully.", GREEN, RESET),
        _ => eprintln!("\n{}[ERROR]{} Command inside container returned an error.", RED, RESET),
    }
}

//fungsi untuk membagi folder host dengan me mount ke kontainer 
// Di src/core/container.rs

pub fn add_shared_folder(name: &str, host_path: &str, container_path: &str) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);
    
    // 1. Ubah path menjadi absolut secara otomatis
    let abs_host_path = std::fs::canonicalize(host_path)
        .expect("Path folder host tidak valid atau tidak ditemukan");

    if Path::new(&config_path).exists() {
        // 2. Cek apakah folder tersebut sudah pernah di-share sebelumnya (hindari duplikasi)
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let mount_entry = format!("lxc.mount.entry = {} {}", abs_host_path.display(), container_path);
        
        if content.contains(&mount_entry) {
            println!("{}[SKIP]{} Folder ini sudah terdaftar di konfigurasi.", YELLOW, RESET);
            return;
        }

        let mut file = OpenOptions::new()
            .append(true)
            .open(&config_path)
            .expect("Gagal membuka config container");

        let mount_config = format!(
            "\n# Shared Folder by MELISA\n\
            lxc.mount.entry = {} {} none bind,create=dir 0 0\n", 
            abs_host_path.display(), container_path
        );

        match file.write_all(mount_config.as_bytes()) {
            Ok(_) => {
                println!("{}[SUCCESS]{} Shared folder integrated to {}.", GREEN, RESET, name);
                println!("{}[IMPORTANT]{} Please run 'melisa --stop {}' and 'melisa --run {}' to apply.", YELLOW, RESET, name, name);
            },
            Err(e) => eprintln!("{}[ERROR]{} Gagal menulis konfigurasi: {}", RED, RESET, e),
        }
    }
}

pub fn remove_shared_folder(name: &str, host_path: &str, container_path: &str) {
    let config_path = format!("{}/{}/config", LXC_PATH, name);
    
    // 1. Standarisasi path host agar match dengan yang ada di config
    let abs_host_path = std::fs::canonicalize(host_path)
        .expect("Path folder host tidak valid atau tidak ditemukan");
    let host_path_str = abs_host_path.to_string_lossy();

    if Path::new(&config_path).exists() {
        let content = std::fs::read_to_string(&config_path)
            .expect("Gagal membaca konfigurasi container");

        let target_entry = format!("lxc.mount.entry = {} {}", host_path_str, container_path);
        let comment_tag = "# Shared Folder by MELISA";

        let lines: Vec<&str> = content.lines().collect();
        let mut new_lines = Vec::new();
        let mut removed = false;

        let mut i = 0;
        while i < lines.len() {
            // Cek apakah baris ini mengandung mount entry yang dicari
            if lines[i].contains(&target_entry) {
                // Opsional: Hapus komentar MELISA jika ada tepat di atas baris entry
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
            println!("{}[SKIP]{} Shared folder tidak ditemukan dalam konfigurasi.", YELLOW, RESET);
            return;
        }

        // 2. Tulis ulang file tanpa baris yang dihapus
        let new_content = new_lines.join("\n");
        match std::fs::write(&config_path, new_content) {
            Ok(_) => {
                println!("{}[SUCCESS]{} Shared folder removed from {}.", GREEN, RESET, name);
                println!("{}[IMPORTANT]{} Please restart the container to apply changes.", YELLOW, RESET);
            },
            Err(e) => eprintln!("{}[ERROR]{} Gagal memperbarui konfigurasi: {}", RED, RESET, e),
        }
    } else {
        eprintln!("{}[ERROR]{} Container config tidak ditemukan.", RED, RESET);
    }
}

//penanganan upload file ke container dengan tarball via stdin
pub fn upload_to_container(name: &str, dest_path: &str) {
    let extract_cmd = format!("mkdir -p {} && tar -xzf - -C {}", dest_path, dest_path);
    
    let status = Command::new("sudo")
        .args(&["lxc-attach", "-P", LXC_PATH, "-n", name, "--", "bash", "-c", &extract_cmd])
        .stdin(Stdio::inherit())  // Menerima file tarbal dari koneksi klien
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => println!("Upload ke '{}' selesai.", dest_path),
        _ => eprintln!("Gagal mengekstrak data di dalam container."),
    }
}

pub fn list_containers(only_active: bool) {
    println!("{}[INFO]{} Listing containers...", GREEN, RESET);
    
    let mut cmd = Command::new("sudo");
    cmd.args(&["lxc-ls", "-P", LXC_PATH, "--fancy"]);

    if only_active {
        cmd.arg("--active");
    }

    let output = cmd.output();

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("{}", String::from_utf8_lossy(&out.stdout));
            } else {
                eprintln!("{}[ERROR]{} Gagal mengambil daftar.", RED, RESET);
            }
        }
        Err(e) => eprintln!("{}[FATAL]{} Error: {}", RED, RESET, e),
    }
}