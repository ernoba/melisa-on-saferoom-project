/// Melisa Project - 2026
/// MIT License

/// Copyright (c) 2026 Erick Adriano Sebastian

/// modul cli berisi fungsi-fungsi untuk menangani antarmuka
/// berbasis command-line (CLI) untuk melisa di sini terdapat 
/// file executor.rs yang bertanggung jawab mengarahkan flag
/// perintah dengan benar ke suatu fungsi
mod cli;

/// modul core berisi fungsi inti untuk pengecekan root 
/// dan fungsi-fungsi penting lainnya yang digunakan 
/// dalam program ini ia menangani bagian penting dari program ini, 
/// seperti pengecekan hak akses root
mod core;

mod distros; // <-- Modul baru untuk manajemen distro LXC

// modul std
use std::process::Command;

// modul CLI untuk melisa
use cli::melisa_cli::melisa;
use cli::wellcome::display_melisa_banner;
use cli::prompt::Prompt;
use cli::executor::execute_command;

// modul pengecekan root
use core::root_check::check_root;

/// fungsi utama - melisa
/// melakuakn check root, jika tidak root maka program akan mencobanya 
/// dengan sudo jika gagal program akan keluar dengan kode error 1
/// Jika argumen pertama adalah "-c", itu berarti dari penggna yang memakai ssh
/// Jika tidak, akan menampilkan banner dan masuk ke CLI interaktif.
fn main() {
    // pengecekan apakah user sudah root
    if !check_root() {

        // Jika tidak root, kita akan mencoba menjalankan 
        // program ini dengan sudo untuk mendapatkan hak akses root
        let args: Vec<String> = std::env::args().skip(1).collect();
        
        // mencoba menjalankan program ini dengan 
        // sudo untuk mendapatkan hak akses root
        let status = Command::new("sudo")
            .arg("-H") 
            .arg("/usr/local/bin/melisa")
            .args(&args)
            .status();

        // Jika berhasil menjalankan sudo, 
        // kita keluar dengan kode yang sama dengan proses sudo
        match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(0)),

            // Jika terjadi error saat menjalankan sudo, 
            // kita anggap eskalasi gagal dan keluar dengan kode error
            Err(_) => {
                eprintln!("Error: Failed to escalate privileges with sudo. Please run this program as root.");

                // keluar dengan kode error 1 untuk 
                // menunjukkan bahwa terjadi kesalahan
                std::process::exit(1);
            }
        }
    }

    // simpan perintah yang diberikan oleh pengguna 
    let args: Vec<String> = std::env::args().collect();

    // lakukan eksekusi perintah jika argumen pertama adalah "-c", 
    // ini berarti perintah diberikan langsung melalui SSH
    if args.len() >= 3 && args[1] == "-c" {
        let cmd_string = &args[2];
        let p_info = Prompt::new();
        
        // eksekusi perintah yang diberikan oleh pengguna melalui SSH
        let _ = execute_command(cmd_string, &p_info.user, &p_info.home);

        // langsung keluar setelah mengeksekusi perintah, 
        // karena ini berarti pengguna hanya ingin 
        // menjalankan satu perintah melalui SSH
        std::process::exit(0); 
    }

    // tampilkan banner dan masuk ke CLI interaktif 
    // jika tidak ada argumen "-c"
    display_melisa_banner();

    // Ambil nama pengguna yang menjalankan sesi ini,
    // jika tidak ditemukan, gunakan "root" sebagai default
    let current_user = std::env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());
    println!("Session ID: {} | Environment: SECURE_JAIL", current_user);

    // jalankan CLI interaktif melisa
    melisa();
}