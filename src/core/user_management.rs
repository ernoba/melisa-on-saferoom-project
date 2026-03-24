use tokio::process::Command;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio;

use crate::core::root_check::{ensure_admin, check_if_admin};
use crate::cli::color_text::{BOLD, GREEN, RED, RESET, YELLOW};

// 1. Definisikan Enum untuk Role
pub enum UserRole {
    Admin,
    Regular,
}

// --- USER MANAGEMENT ---

pub async fn add_melisa_user(username: &str) {
    if !ensure_admin().await { return; } 
    println!("\n{}--- Adding New Melisa User: {} ---{}", BOLD, username, RESET);

    // Langkah 1: Tanya Role
    println!("{}Select Role for {}:{}", BOLD, username, RESET);
    println!("  1) Admin (Full Management: Users, Projects, & LXC)");
    println!("  2) Regular (Project & LXC Management Only)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush().await;

    let mut choice = String::new();
    let mut reader = BufReader::new(io::stdin());
    let _ = reader.read_line(&mut choice).await;

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular,
    };

    // Langkah 2: Buat User Sistem dengan shell melisa
    // Kita gunakan -m untuk home, dan -s untuk shell kustom
    let status = Command::new("useradd")
        .args(&["-m", "-s", "/usr/local/bin/melisa", username])
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} User '{}' created.", GREEN, RESET, username);

            // EKSEKUSI ISOLASI FOLDER
            // 700 memastikan user lain tidak bisa mengintip folder ini
            let folder_path = format!("/home/{}", username);
            let _ = Command::new("chmod")
                .args(&["700", &folder_path])
                .status()
                .await;

            // Set Password & Konfigurasi Sudoers
            if set_user_password(username).await {
                configure_sudoers(username, role).await;
            }
        }
        _ => {
            eprintln!("{}[ERROR]{} Gagal membuat user. Mungkin user sudah ada.", RED, RESET);
        }
    }
}

pub async fn set_user_password(username: &str) -> bool {
    println!("{}[ACTION]{} Please set password for {}:", YELLOW, RESET, username);
    
    // Kita biarkan passwd berjalan secara interaktif di terminal
    let status = Command::new("passwd")
        .arg(username)
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} Password updated for {}.", GREEN, RESET, username);
            true
        },
        _ => {
            eprintln!("{}[ERROR]{} Gagal menyetel password.", RED, RESET);
            false
        }
    }
}

pub async fn delete_melisa_user(username: &str) {
    if !ensure_admin().await { return; }
    println!("\n{}--- Deleting User: {} ---{}", BOLD, username, RESET);

    // 1. Matikan semua proses user agar tidak 'busy' saat dihapus
    println!("{}[INFO]{} Terminating all processes for user '{}'...", YELLOW, RESET, username);
    let _ = Command::new("pkill").args(&["-u", username]).status().await;

    // 2. Hapus user beserta home directory (-r)
    let status_del = Command::new("userdel")
        .args(&["-r", "-f", username])
        .status()
        .await;

    // 3. Hapus file sudoers spesifik user tersebut
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);
    let status_rm = Command::new("rm")
        .args(&["-f", &sudoers_path])
        .status()
        .await;

    match (status_del, status_rm) {
        (Ok(s1), Ok(s2)) if s1.success() && s2.success() => {
            println!("{}[SUCCESS]{} User '{}' and permissions removed.", GREEN, RESET, username);
        },
        _ => {
            eprintln!("{}[ERROR]{} Penghapusan tidak sempurna (User mungkin sudah tidak ada).", RED, RESET);
        }
    }
}

async fn configure_sudoers(username: &str, role: UserRole) {
    // Daftar perintah dasar yang dibutuhkan untuk operasional Git dan LXC
    let mut commands = vec![
        "/usr/sbin/lxc-*",
        "/usr/bin/git *",
        "/usr/sbin/git *",
        "/usr/local/bin/melisa *"
    ];

    match role {
        UserRole::Admin => {
            // Admin mendapatkan akses ke manajemen user dan sistem
            commands.extend(vec![
                "/usr/sbin/useradd *", "/usr/sbin/userdel *", "/usr/bin/passwd *",
                "/usr/bin/pkill *", "/usr/bin/grep *", "/usr/bin/lxc-info *",
                "/usr/bin/ls /etc/sudoers.d/", "/usr/bin/rm -f /etc/sudoers.d/melisa_*",
                "/usr/bin/tee /etc/sudoers.d/melisa_*",
                "/usr/bin/chmod *", "/usr/sbin/chmod *", 
                "/usr/bin/chown *", "/usr/sbin/chown *",
                "/usr/bin/mkdir *"
            ]);
        },
        UserRole::Regular => {
            // Regular user hanya memiliki akses dasar (sudah didefinisikan di awal)
        }
    }

    // Buat rule sudoers: NOPASSWD penting agar CLI melisa tidak meminta password berulang kali
    let sudoers_rule = format!("{} ALL=(ALL) NOPASSWD: {}\n", username, commands.join(", "));
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);

    // Gunakan tee untuk menulis ke folder sistem yang diproteksi
    let mut child = Command::new("tee")
        .arg(&sudoers_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("Failed to write sudoers file");

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(sudoers_rule.as_bytes()).await;
    }
    let _ = child.wait().await;

    // Set permission sudoers file ke 0440 (Read-only for root)
    let _ = Command::new("chmod").args(&["0440", &sudoers_path]).status().await;
}

pub async fn list_melisa_users() {
    if !ensure_admin().await { return; }
    println!("\n{}--- Registered Melisa Users ---{}", BOLD, RESET);

    // Cari user yang menggunakan shell melisa di /etc/passwd
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output()
        .await;

    let mut existing_users = Vec::new();

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(user) = line.split(':').next() {
                existing_users.push(user.to_string());
                let tag = if check_if_admin(user).await { 
                    format!("{}[ADMIN]{}", GREEN, RESET) 
                } else { 
                    format!("{}[USER]{}", YELLOW, RESET) 
                };
                println!("  > {:<15} {}", user, tag);
            }
        }
    }

    // Pengecekan Sampah Konfigurasi (Orphaned Sudoers)
    println!("\n{}--- Checking for Orphaned Sudoers (Trash) ---{}", BOLD, RESET);
    
    let sudoers_files = Command::new("ls")
        .args(&["/etc/sudoers.d/"])
        .output()
        .await;
    
    match sudoers_files {
        Ok(out) if out.status.success() => {
            let files = String::from_utf8_lossy(&out.stdout);
            let mut found_trash = false;

            for file in files.lines() {
                if file.starts_with("melisa_") {
                    let user_from_file = file.replace("melisa_", "");
                    if !existing_users.contains(&user_from_file) {
                        println!("  {}! Found trash:{} {} (User already deleted)", RED, RESET, file);
                        found_trash = true;
                    }
                }
            }
            if !found_trash { 
                println!("  {}No trash found. System is clean.{}", GREEN, RESET); 
            }
        },
        _ => println!("{}[ERROR]{} Gagal mengakses /etc/sudoers.d/.", RED, RESET),
    }
}

pub async fn upgrade_user(username: &str) {
    if !ensure_admin().await { return; }
    println!("\n{}--- Upgrading User Permissions: {} ---{}", BOLD, username, RESET);

    // Pastikan user ada di sistem
    let check_user = Command::new("id").arg(username).output().await;
    if let Ok(out) = check_user {
        if !out.status.success() {
            eprintln!("{}[ERROR]{} User '{}' tidak ditemukan.", RED, RESET, username);
            return;
        }
    }

    println!("Select New Role for {}:", username);
    println!("  1) Admin (Full Access)");
    println!("  2) Regular (LXC & Project Only)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush().await;

    let mut choice = String::new();
    let mut reader = BufReader::new(io::stdin());
    let _ = reader.read_line(&mut choice).await;

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular,
    };

    configure_sudoers(username, role).await;
    println!("{}[DONE]{} Izin user '{}' telah diperbarui.", GREEN, RESET, username);
}

pub async fn clean_orphaned_sudoers() {
    if !ensure_admin().await { return; }
    println!("\n{}--- Cleaning Orphaned Sudoers ---{}", BOLD, RESET);
    
    // 1. Ambil daftar user melisa yang valid
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output()
        .await;

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        let existing_users: Vec<&str> = result.lines()
            .map(|l| l.split(':').next().unwrap_or(""))
            .collect();

        // 2. Scan folder sudoers
        let files_out = Command::new("ls").args(&["/etc/sudoers.d/"]).output().await;
        
        if let Ok(f_out) = files_out {
            let files = String::from_utf8_lossy(&f_out.stdout);
            for file in files.lines() {
                if file.starts_with("melisa_") {
                    let user_name = file.replace("melisa_", "");
                    if !existing_users.contains(&user_name.as_str()) {
                        println!("{}[CLEANING]{} Removing orphaned config: {}", YELLOW, RESET, file);
                        let _ = Command::new("rm").args(&["-f", &format!("/etc/sudoers.d/{}", file)]).status().await;
                    }
                }
            }
        }
    }
    println!("{}[DONE]{} Garbage collection finished.", GREEN, RESET);
}