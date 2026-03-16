use std::process::Command;
use std::env;
use crate::cli::color_text::{RED, RESET};

pub fn check_root() -> bool {
    // Pastikan sudah menambahkan 'libc = "0.2"' di Cargo.toml
    unsafe { libc::geteuid() == 0 }
}

pub fn check_if_admin(username: &str) -> bool {
    let sudoers_path = format!("/etc/sudoers.d/melisa_{}", username);
    
    let check_admin = Command::new("sudo")
        .arg("-n") // <--- KUNCI SAKTINYA DI SINI (Non-interactive)
        .args(&["/usr/bin/grep", "-qs", "useradd", &sudoers_path])
        .status();

    match check_admin {
        // Jika sukses (0), berarti dia Admin dan punya izin NOPASSWD
        Ok(s) if s.success() => true,
        // Jika gagal (karena nggak ada izin atau perlu password), langsung anggap bukan Admin
        _ => false, 
    }
}

// Fungsi untuk mengecek apakah user yang sedang menjalankan aplikasi adalah Admin
pub fn ensure_admin() -> bool {
    // Ambil nama user yang sedang login/menjalankan binary
    let current_user = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    
    if !check_if_admin(&current_user) {
        println!("{}[ERROR] Permission not allowed. This function is for admin only.{}", RED, RESET);
        return false;
    }
    true
}