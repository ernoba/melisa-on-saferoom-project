use std::process::Command;
use std::process::Stdio; // Untuk attach ke container dengan interaktif
use crate::cli::color_text::{BOLD, GREEN, RED, RESET, YELLOW}; // Tambah YELLOW biar lebih pas buat warning

pub fn create_new_container(name: &str) {
    println!("{}--- Creating Container: {} ---{}", BOLD, name, RESET);

    // 1. Pakai .output() supaya kita bisa baca isi pesan error-nya (stderr)
    let process = Command::new("lxc-create")
        .args(&[
            "-t", "download", 
            "-n", name, 
            "--", 
            "-d", "debian", 
            "-r", "bookworm", 
            "-a", "amd64"
        ])
        .output(); // Kita tangkap hasilnya di sini

    match process {
        Ok(output) => {
            if output.status.success() {
                // Kasus: Berhasil
                println!("{}[SUCCESS]{} Container '{}' created successfully.", GREEN, RESET, name);
            } else {
                // Kasus: Gagal, kita cek kenapa gagalnya
                let error_msg = String::from_utf8_lossy(&output.stderr);

                if error_msg.contains("already exists") {
                    // Kasus spesifik: Container sudah ada
                    println!("{}[WARNING]{} Container '{}' already exists.", YELLOW, RESET, name);
                } else {
                    // Kasus: Error lainnya
                    eprintln!("{}[ERROR]{} Failed to create container '{}'.{}", RED, RESET, name, RESET);
                    eprintln!("Details: {}", error_msg);
                }
            }
        },
        Err(e) => {
            eprintln!("{}[FATAL]{} Could not run lxc-create: {}", RED, RESET, e);
        }
    }
}

pub fn delete_container(name: &str) {
    println!("{}--- Deleting Container: {} ---{}", BOLD, name, RESET);

    let process = Command::new("lxc-destroy")
        .args(&[
            "-f",      
            "-n", name 
        ])
        .output();

    match process {
        Ok(output) => {
            if output.status.success() {
                println!("{}[SUCCESS]{} Container '{}' deleted successfully.", GREEN, RESET, name);
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                
                // Cek jika errornya karena kontainer memang tidak ada
                if error_msg.contains("does not exist") {
                    println!("{}[INFO]{} Container '{}' does not exist, so no need to delete.", YELLOW, RESET, name);
                } else {
                    eprintln!("{}[ERROR]{} Failed to delete container '{}'.{}", RED, RESET, name, RESET);
                    eprintln!("Details: {}", error_msg);
                }
            }
        },
        Err(e) => {
            eprintln!("{}[FATAL]{} Could not run lxc-destroy: {}", RED, RESET, e);
        }
    }
}

// start container (background)
pub fn start_container(name: &str) {
    println!("{}[INFO]{} Starting container '{}' in background...", GREEN, RESET, name);
    
    let status = Command::new("lxc-start")
        .args(&["-n", name, "-d"]) 
        .status();

    match status {
        Ok(s) if s.success() => println!("{}[SUCCESS]{} Container is now running.", GREEN, RESET),
        Ok(s) if s.code() == Some(1) => println!("{}[INFO]{} Container is already running.", YELLOW, RESET),
        _ => eprintln!("{}[ERROR]{} Failed to start container.", RED, RESET),
    }
}

// attach ke container (interaktif)
pub fn attach_to_container(name: &str) {
    println!("{}[MODE]{} Entering Saferoom: {}. Type 'exit' to return to MELISA.", BOLD, name, RESET);

    let status = Command::new("lxc-attach")
        .args(&["-n", name, "--", "bash"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .status();

    if let Ok(s) = status {
        if s.success() {
            println!("\n{}[BACK]{} Returned to MELISA CLI.", GREEN, RESET);
        }
    }
}

// kill container
pub fn stop_container(name: &str) {
    println!("{}[SHUTDOWN]{} Stopping container '{}'...", YELLOW, RESET, name);

    // lxc-stop akan mencoba mengirim sinyal shutdown secara halus terlebih dahulu
    let process = Command::new("lxc-stop")
        .args(&["-n", name])
        .output();

    match process {
        Ok(output) => {
            if output.status.success() {
                println!("{}[SUCCESS]{} Container '{}' has been stopped.", GREEN, RESET, name);
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                
                // Cek jika ternyata kontainer memang sudah mati
                if error_msg.contains("is not running") {
                    println!("{}[INFO]{} Container '{}' is not running.", YELLOW, RESET, name);
                } else {
                    eprintln!("{}[ERROR]{} Failed to stop container '{}'.", RED, RESET, name);
                    eprintln!("Details: {}", error_msg);
                }
            }
        },
        Err(e) => {
            eprintln!("{}[FATAL]{} Could not run lxc-stop: {}", RED, RESET, e);
        }
    }
}

// run command in container (non-interaktif)
pub fn send_command(name: &str, command_args: &[&str]) {
    if command_args.is_empty() {
        eprintln!("{}[ERROR]{} No command provided to send.", RED, RESET);
        return;
    }

    println!("{}[SEND]{} Executing on '{}' {}...", BOLD, name, command_args.join(" "), RESET);

    // Kita gunakan lxc-attach -n <name> -- <command>
    let status = Command::new("lxc-attach")
        .arg("-n")
        .arg(name)
        .arg("--")
        .args(command_args) // Mengirimkan sisa argumen sebagai perintah
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => println!("\n{}[DONE]{} Command executed successfully.", GREEN, RESET),
        _ => eprintln!("\n{}[ERROR]{} Command failed or container is not running.", RED, RESET),
    }
}

//list container
pub fn list_containers(only_active: bool) {
    println!("{}[INFO]{} Listing {}containers...", GREEN, RESET, if only_active { "active " } else { "" });
    
    let mut cmd = Command::new("lxc-ls");
    cmd.arg("--fancy");

    if only_active {
        cmd.arg("--active");
    }

    // Gunakan .output() alih-alih .status() untuk menangkap data
    let output = cmd.output();

    match output {
        Ok(out) => {
            if out.status.success() {
                let result = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = result.trim().lines().collect();
                
                if lines.len() <= 1 {
                    println!("{}[-]{} Tidak ada kontainer yang {}ditemukan.", YELLOW, RESET, if only_active { "aktif " } else { "" });
                } else {
                    println!("{}", result);
                }
            } else {
                let err = String::from_utf8_lossy(&out.stderr);
                eprintln!("{}[ERROR]{} Gagal mengambil daftar: {}", RED, RESET, err);
            }
        }
        Err(e) => eprintln!("{}[FATAL]{} Could not run lxc-ls: {}", RED, RESET, e),
    }
}