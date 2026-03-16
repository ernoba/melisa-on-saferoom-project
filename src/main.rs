mod cli;
mod core;

use std::process::Command;
use cli::melisa_cli::melisa;
use cli::wellcome::display_melisa_banner;
use core::root_check::check_root;

fn main() {
    // 1. SELF-ESCALATION DENGAN CLEAN ENVIRONMENT
    if !check_root() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        
        // Gunakan 'sudo -H' untuk mengganti HOME ke /root
        let status = Command::new("sudo")
            .arg("-H") 
            .arg("/usr/local/bin/melisa")
            .args(&args)
            .status();

        match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(0)),
            Err(_) => {
                eprintln!("Error: Eskalasi hak akses gagal.");
                std::process::exit(1);
            }
        }
    }

    // 2. LOGIKA UTAMA (Sekarang berjalan sebagai Root dengan HOME=/root)
    display_melisa_banner();
    
    let current_user = std::env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());
    println!("Session ID: {} | Environment: SECURE_JAIL", current_user);

    melisa();
}