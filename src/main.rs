/// Melisa Project - 2026
/// MIT License
/// Copyright (c) 2026 Erick Adriano Sebastian

mod cli;
mod core;
mod distros; 

use tokio::process::Command; // Gunakan Command dari tokio
use std::process::exit;      // Gunakan exit dari std

// Import modul kamu
use cli::melisa_cli::melisa;
use cli::wellcome::display_melisa_banner;
use cli::prompt::Prompt;
use cli::executor::execute_command;
use core::root_check::check_root;
pub mod deployment; 

// get metadata Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[tokio::main] 
async fn main() {
    // 1. Pengecekan Root & Eskalasi
    if !check_root() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        
        // Tambahkan .await di sini karena tokio::process::Command itu async
        let status = Command::new("sudo")
            .arg("-H") 
            .arg("/usr/local/bin/melisa")
            .args(&args)
            .status()
            .await; // <--- PENTING: Harus di-await

        match status {
            Ok(s) => {
                // Gunakan std::process::exit
                exit(s.code().unwrap_or(0));
            }
            Err(_) => {
                eprintln!("Error: Failed to escalate privileges with sudo.");
                exit(1);
            }
        }
    }

    // 2. Logika Argumen CLI
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 {
        let cmd_string = if args.len() >= 3 && args[1] == "-c" {
            // Jika dipanggil via SSH (e.g., melisa -c "command args")
            args[2..].join(" ") 
        } else {
            // Jika dipanggil langsung dengan argumen
            args[1..].join(" ")
        };

        if !cmd_string.is_empty() {
            let p_info = Prompt::new();
            let _ = execute_command(&cmd_string, &p_info.user, &p_info.home).await;
            exit(0);
        }
    }

    // 3. Mode Interaktif
    display_melisa_banner();

    let current_user = std::env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());
    println!("Session ID: {} | Environment: SECURE_JAIL", current_user);

    // 4. Perbaikan panggil fungsi melisa
    // Asumsi: melisa() adalah fungsi async
    melisa().await; 
}

#[cfg(test)]
mod tests;