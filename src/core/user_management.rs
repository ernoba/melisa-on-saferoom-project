use std::process::{Command, Stdio};
use std::io::{self, Write};

use crate::core::root_check::ensure_admin;
use crate::core::root_check::check_if_admin;
use crate::cli::color_text::{BOLD, GREEN, RED, RESET, YELLOW}; // Tambah YELLOW biar lebih pas buat warning

// 1. Definisikan Enum untuk Role
pub enum UserRole {
    Admin,
    Regular,
}

//user management
pub fn add_melisa_user(username: &str) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Adding New Melisa User: {} ---{}", BOLD, username, RESET);

    // Langkah 1: Tanya Role
    println!("{}Select Role for {}:{}", BOLD, username, RESET);
    println!("  1) Admin (Can manage users & LXC)");
    println!("  2) Regular (Can only manage LXC)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).expect("Failed to read input");

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular, // Default ke Regular jika input salah
    };

    // Langkah 2: Buat User Sistem
    let status = Command::new("sudo")
        .args(&["/usr/sbin/useradd", "-m", "-s", "/usr/local/bin/melisa", username])
        .status();

    if let Ok(s) = status {
        if s.success() {
            println!("{}[SUCCESS]{} User '{}' created.", GREEN, RESET, username);
            
            // Langkah 3: Set Password
            if set_user_password(username) {
                // Langkah 4: Konfigurasi Sudoers berdasarkan Role
                configure_sudoers(username, role);
            }
        } else {
            eprintln!("{}[ERROR]{} Failed to create user.", RED, RESET);
        }
    }
}

// Fungsi pembantu untuk set password
pub fn set_user_password(username: &str) -> bool {
    println!("{}[ACTION]{} Please set password for {}:", YELLOW, RESET, username);
    let status = Command::new("sudo")
        .arg("passwd")
        .arg(username)
        .status();

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

pub fn delete_melisa_user(username: &str) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Deleting User: {} ---{}", BOLD, username, RESET);

    // 1. PAKSA: Usir user dan matikan semua prosesnya (SSH, Bash, dll)
    println!("{}[INFO]{} Terminating all processes for user '{}'...", YELLOW, RESET, username);
    let _ = Command::new("sudo").args(&["/usr/bin/pkill", "-u", username]).status();

    // 2. Hapus user sistem
    let status_del = Command::new("sudo")
        .args(&["/usr/sbin/userdel", "-r", "-f", username]) // Tambah -f (force)
        .status();

    // 3. Hapus file sudoers
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);
    let status_rm = Command::new("sudo")
        .args(&["/usr/bin/rm", "-f", &sudoers_path])
        .status();

    match (status_del, status_rm) {
        (Ok(s1), Ok(s2)) if s1.success() && s2.success() => {
            println!("{}[SUCCESS]{} User '{}' and permissions removed.", GREEN, RESET, username);
        },
        _ => {
            eprintln!("{}[ERROR]{} Gagal menghapus total. Mungkin user sedang digunakan atau sudah hilang.", RED, RESET);
        }
    }
}

fn configure_sudoers(username: &str, role: UserRole) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    let mut commands = vec![
        "/usr/sbin/lxc-*", // Izinkan semua sub-command lxc
    ];

    match role {
        UserRole::Admin => {
            // Kita gunakan "*" agar flexibel terhadap flag (seperti -f, -m, -r)
            commands.push("/usr/sbin/useradd *");
            commands.push("/usr/sbin/userdel *");
            commands.push("/usr/bin/passwd *");
            commands.push("/usr/bin/pkill *");
            commands.push("/usr/bin/grep *");
            commands.push("/usr/sbin/lxc-*"); // Ini biasanya sudah mencakup lxc-info
            commands.push("/usr/bin/lxc-info *"); // Tambahkan secara spesifik jika perlu
            commands.push("/usr/bin/ls /etc/sudoers.d/"); // Harus sama persis dengan panggilan di Rust
            commands.push("/usr/bin/rm -f /etc/sudoers.d/melisa_*"); // Match persis dengan kode
            commands.push("/usr/bin/tee /etc/sudoers.d/melisa_*");
        },
        UserRole::Regular => {}
    }

    let sudoers_rule = format!("{} ALL=(root) NOPASSWD: {}\n", username, commands.join(", "));
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);

    // Proses tulis file dengan sudo tee...
    let mut child = Command::new("sudo")
        .args(&["/usr/bin/tee", &sudoers_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("Failed to spawn sudo tee");

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(sudoers_rule.as_bytes());
    }
    child.wait().unwrap();
}

pub fn list_melisa_users() {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Registered Melisa Users ---{}", BOLD, RESET);

    // 1. Ambil daftar user asli
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output();

    let mut existing_users = Vec::new();

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(user) = line.split(':').next() {
                existing_users.push(user.to_string());
                let tag = if check_if_admin(user) { 
                    format!("{}[ADMIN]{}", GREEN, RESET) 
                } else { 
                    format!("{}[USER]{}", YELLOW, RESET) 
                };
                println!("  > {:<15} {}", user, tag);
            }
        }
    }

    // 2. LOGIKA JANITOR dengan Error Handling yang Jujur
    println!("\n{}--- Checking for Orphaned Sudoers (Trash) ---{}", BOLD, RESET);
    
    // Pastikan path /usr/bin/ls ini SAMA PERSIS dengan yang ada di file sudoers
    let sudoers_files = Command::new("sudo")
        .args(&["/usr/bin/ls", "/etc/sudoers.d/"])
        .output();
    
    match sudoers_files {
        Ok(out) if out.status.success() => {
            let files = String::from_utf8_lossy(&out.stdout);
            let mut found_trash = false;

            for file in files.lines() {
                if file.starts_with("melisa_") {
                    let user_from_file = file.replace("melisa_", "");
                    // Cukup gunakan &user_from_file karena user_from_file adalah String
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
        _ => {
            // Jika masuk ke sini, berarti sudo minta password atau ditolak
            println!("{}[ERROR]{} Akses ditolak saat memeriksa sudoers. Pastikan izin NOPASSWD benar.", RED, RESET);
        }
    }
}

pub fn upgrade_user(username: &str) {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Upgrading User Permissions: {} ---{}", BOLD, username, RESET);

    // Cek dulu apakah usernya memang ada di sistem
    let check_user = Command::new("id").arg(username).output();
    if let Ok(out) = check_user {
        if !out.status.success() {
            eprintln!("{}[ERROR]{} User '{}' tidak ditemukan di sistem.", RED, RESET, username);
            return;
        }
    }

    println!("Select New Role for {}:", username);
    println!("  1) Admin (Full Access)");
    println!("  2) Regular (LXC Only)");
    print!("Choose (1/2): ");
    let _ = io::stdout().flush();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();

    let role = match choice.trim() {
        "1" => UserRole::Admin,
        _ => UserRole::Regular,
    };

    // Panggil fungsi konfigurasi sudoers yang sudah kita buat tadi
    configure_sudoers(username, role);
    println!("{}[DONE]{} Izin user '{}' telah diperbarui.", GREEN, RESET, username);
}

pub fn clean_orphaned_sudoers() {
    if !ensure_admin() { return; } // Gerbang Keamanan
    println!("{}--- Cleaning Orphaned Sudoers ---{}", BOLD, RESET);
    
    // Gunakan match, jangan unwrap
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output();

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        let existing_users: Vec<&str> = result.lines()
            .map(|l| l.split(':').next().unwrap_or(""))
            .collect();

        let files_out = Command::new("sudo").args(&["/usr/bin/ls", "/etc/sudoers.d/"]).output();
        
        if let Ok(f_out) = files_out {
            let files = String::from_utf8_lossy(&f_out.stdout);
            for file in files.lines() {
                if file.starts_with("melisa_") {
                    let user_name = file.replace("melisa_", "");
                    if !existing_users.contains(&user_name.as_str()) {
                        println!("{}[CLEANING]{} Removing: {}", YELLOW, RESET, file);
                        let _ = Command::new("sudo").args(&["/usr/bin/rm", "-f", &format!("/etc/sudoers.d/{}", file)]).status();
                    }
                }
            }
        }
    }
    println!("{}[DONE]{} System is clean.", GREEN, RESET);
}