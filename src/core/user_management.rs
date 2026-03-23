use tokio::process::Command;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio; // Penting: Stdio tetap dari std

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
    println!("{}--- Adding New Melisa User: {} ---{}", BOLD, username, RESET);

    // Langkah 1: Tanya Role
    println!("{}Select Role for {}:{}", BOLD, username, RESET);
    println!("  1) Admin (Can manage users & LXC)");
    println!("  2) Regular (Can only manage LXC)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush().await;

    let mut choice = String::new();
    let mut reader = BufReader::new(io::stdin());
    reader.read_line(&mut choice).await.expect("Failed to read input");

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular,
    };

    // Langkah 2: Buat User Sistem
    let status = Command::new("sudo")
        .args(&["/usr/sbin/useradd", "-m", "-s", "/usr/local/bin/melisa", username])
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} User '{}' created.", GREEN, RESET, username);

            // EKSEKUSI ISOLASI FOLDER
            let folder_path = format!("/home/{}", username);
            let _ = Command::new("sudo")
                .args(&["chmod", "700", &folder_path])
                .status()
                .await;

            if set_user_password(username).await {
                configure_sudoers(username, role).await;
            }
        }
        _ => {
            eprintln!("{}[ERROR]{} Failed to create user.", RED, RESET);
        }
    }
}

pub async fn set_user_password(username: &str) -> bool {
    println!("{}[ACTION]{} Please set password for {}:", YELLOW, RESET, username);
    // Jalankan passwd secara interaktif
    let status = Command::new("sudo")
        .arg("passwd")
        .arg(username)
        .status()
        .await; // Tambahkan .await

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} Password updated for {}.", GREEN, RESET, username);
            true
        },
        _ => {
            eprintln!("{}[ERROR]{} Failed to set password.", RED, RESET);
            false
        }
    }
}

pub async fn delete_melisa_user(username: &str) {
    if !ensure_admin().await { return; }
    println!("{}--- Deleting User: {} ---{}", BOLD, username, RESET);

    println!("{}[INFO]{} Terminating all processes for user '{}'...", YELLOW, RESET, username);
    let _ = Command::new("sudo").args(&["/usr/bin/pkill", "-u", username]).status().await;

    let status_del = Command::new("sudo")
        .args(&["/usr/sbin/userdel", "-r", "-f", username])
        .status()
        .await;

    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);
    let status_rm = Command::new("sudo")
        .args(&["/usr/bin/rm", "-f", &sudoers_path])
        .status()
        .await;

    match (status_del, status_rm) {
        (Ok(s1), Ok(s2)) if s1.success() && s2.success() => {
            println!("{}[SUCCESS]{} User '{}' and permissions removed.", GREEN, RESET, username);
        },
        _ => {
            eprintln!("{}[ERROR]{} Gagal menghapus total. Mungkin user sedang digunakan atau sudah hilang.", RED, RESET);
        }
    }
}

async fn configure_sudoers(username: &str, role: UserRole) {
    // Tambahkan git ke daftar perintah dasar agar 'melisa --update' bisa jalan
    let mut commands = vec![
        "/usr/sbin/lxc-*",
        "/usr/bin/git *",
        "/usr/sbin/git *"
    ];

    match role {
        UserRole::Admin => {
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
            // Role regular sekarang mewarisi izin git di atas
        }
    }

    // Perhatikan: Kita izinkan user menjalankan git sebagai DIRINYA SENDIRI (afira)
    // agar sinkronisasi folder home tidak bermasalah dengan owner
    let sudoers_rule = format!("{} ALL=(ALL) NOPASSWD: {}\n", username, commands.join(", "));
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);

    let mut child = Command::new("sudo")
        .args(&["/usr/bin/tee", &sudoers_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("Failed to spawn sudo tee");

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(sudoers_rule.as_bytes()).await;
    }
    let _ = child.wait().await;
}

pub async fn list_melisa_users() {
    if !ensure_admin().await { return; }
    println!("{}--- Registered Melisa Users ---{}", BOLD, RESET);

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

    println!("\n{}--- Checking for Orphaned Sudoers (Trash) ---{}", BOLD, RESET);
    
    let sudoers_files = Command::new("sudo")
        .args(&["/usr/bin/ls", "/etc/sudoers.d/"])
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
        _ => println!("{}[ERROR]{} Akses ditolak saat memeriksa sudoers.", RED, RESET),
    }
}

pub async fn upgrade_user(username: &str) {
    if !ensure_admin().await { return; }
    println!("{}--- Upgrading User Permissions: {} ---{}", BOLD, username, RESET);

    let check_user = Command::new("id").arg(username).output().await;
    if let Ok(out) = check_user {
        if !out.status.success() {
            eprintln!("{}[ERROR]{} User '{}' tidak ditemukan.", RED, RESET, username);
            return;
        }
    }

    println!("Select New Role for {}:", username);
    println!("  1) Admin (Full Access)");
    println!("  2) Regular (LXC Only)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush().await;

    let mut choice = String::new();
    let mut reader = BufReader::new(io::stdin());
    reader.read_line(&mut choice).await.unwrap();

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular,
    };

    configure_sudoers(username, role).await;
    println!("{}[DONE]{} Izin user '{}' telah diperbarui.", GREEN, RESET, username);
}

pub async fn clean_orphaned_sudoers() {
    if !ensure_admin().await { return; }
    println!("{}--- Cleaning Orphaned Sudoers ---{}", BOLD, RESET);
    
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output()
        .await;

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        let existing_users: Vec<&str> = result.lines()
            .map(|l| l.split(':').next().unwrap_or(""))
            .collect();

        let files_out = Command::new("sudo").args(&["/usr/bin/ls", "/etc/sudoers.d/"]).output().await;
        
        if let Ok(f_out) = files_out {
            let files = String::from_utf8_lossy(&f_out.stdout);
            for file in files.lines() {
                if file.starts_with("melisa_") {
                    let user_name = file.replace("melisa_", "");
                    if !existing_users.contains(&user_name.as_str()) {
                        println!("{}[CLEANING]{} Removing: {}", YELLOW, RESET, file);
                        let _ = Command::new("sudo").args(&["/usr/bin/rm", "-f", &format!("/etc/sudoers.d/{}", file)]).status().await;
                    }
                }
            }
        }
    }
    println!("{}[DONE]{} System is clean.", GREEN, RESET);
}