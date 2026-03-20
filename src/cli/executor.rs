use tokio::process::Command; // Gunakan tokio
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt}; // Untuk async stdin/stdout
use std::env;

use crate::core::container::*;
use crate::core::setup::install;
use crate::core::root_check::admin_check;
use crate::distros::distro::get_lxc_distro_list;
use crate::cli::loading::execute_with_spinner;
use crate::cli::color_text::{RED, GREEN, YELLOW, BOLD, RESET};
use crate::core::user_management::{
    add_melisa_user, set_user_password, delete_melisa_user, 
    list_melisa_users, upgrade_user, clean_orphaned_sudoers
};
use crate::core::project_management::{PROJECTS_MASTER, delete_project, invite, list_projects, new_project, out_user, pull, update_project};

pub enum ExecResult {
    Continue,
    Break,
    ResetHistory,
    Error(String),
}

// 1. Ubah menjadi async fn
pub async fn execute_command(input: &str, user: &str, home: &str) -> ExecResult {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() { return ExecResult::Continue; }

    match parts[0] {
        "melisa" => {
            let sub_cmd = parts.get(1).map(|&s| s).unwrap_or("");

            match sub_cmd {
                "--help" | "-h" => {
                    if !admin_check().await {
                        println!("{}Usage: melisa [options]{}", BOLD, RESET);
                        println!("Options:");
                        println!("  --help             Show this help message");
                        println!("  --run <name>       Run a command inside a container");

                     }// Gerbang Keamanan
                     else {
                        println!("{}Usage: melisa [options]{}", BOLD, RESET);
                        println!("Options:");
                        println!("  --help             Show this help message");
                        println!("  --setup            Setup LXC environment (install dependencies, etc.)");
                        println!("  --clear            Clear history data/history.txt");
                        println!("  --search <keyword> Search available LXC distros by keyword");
                        println!("  --create <name> <distro_code>  Create a new LXC container");
                        println!("  --delete <name>    Delete an existing LXC container");
                        println!("  --run <name>       Run a command inside a container");
                        println!("  --use <name>       Attach to a container interactively");
                        println!("  --stop <name>      Stop a running container");
                        println!("  --list             List all containers");
                        println!("  --active           List only active (running) containers");
                        println!("  --add <user>       Add a user to Melisa access list");
                        println!("  --remove <user>    Remove a user from Melisa access list");
                        println!("  --users            List all users with Melisa access");
                        println!("  --upgrade <user>   Upgrade a user's permissions (e.g., to sudo)");
                        println!("  --new_project <name>  Create a new master project (Admin only)");
                        println!("  --delete_project <name> remove your project");
                        println!("  --invite <project> <user1> <user2> ...  Invite users to a project (Admin only)");
                        println!("  --out <project> <user1> <user2> ...  Out users from project");
                        println!("  --projects         Show all projects");
                        println!("  --pull <from_user> <project_name>  merge code from user to master");
                        println!("  --clean            Clean orphaned sudoers files for non-existent users");
                        println!("  --upload <name> <dest_path>  Upload a file to a container");
                        println!("  --share <name> <host_path> <cont_path>  Share a folder between host and container");
                        println!("  --reshare <name> <host_path> <cont_path>  Remove a shared folder between host and container");
                     }
                },
                "--setup" => {
                    install().await;
                },
                "--clear" => {
                    // Panggil fungsi simpel tadi
                    if !admin_check().await {
                        println!("{}[ERROR]{} You don't have permission to clear history.{}", RED, RESET, user);
                        return ExecResult::Continue;
                    }
                    return ExecResult::ResetHistory
                },
                "--search" => {
                    let keyword = parts.get(2).unwrap_or(&"").to_lowercase();
                    
                    // Ambil tuple (data, is_cache)
                    let (list, is_cache) = execute_with_spinner(
                        "Sedang menyinkronkan daftar distro...", 
                        get_lxc_distro_list()
                    ).await;

                    // Beri tahu user sumber datanya
                    if is_cache {
                        println!("{}[CACHE]{} Menampilkan data lokal (Offline Mode).", YELLOW, RESET);
                    } else {
                        println!("{}[FRESH]{} Berhasil menyinkronkan daftar terbaru dari server.", GREEN, RESET);
                    }

                    println!("\n{:<20} | {:<10} | {:<10}", "KODE UNIK", "DISTRO", "ARCH");
                    println!("{}", "-".repeat(45)); // Garis pemisah agar rapi

                    for d in list {
                        if d.slug.contains(&keyword) || d.name.contains(&keyword) {
                            println!("{:<20} | {:<10} | {:<10}", d.slug, d.name, d.arch);
                        }
                    }
                },

                "--create" => {
                    let name = parts.get(2).unwrap_or(&"");
                    let code = parts.get(3).unwrap_or(&"");

                    if name.is_empty() || code.is_empty() {
                        println!("{}[ERROR]{} Nama dan Kode Distro harus diisi!", RED, RESET);
                        return ExecResult::Continue;
                    }

                    // Ambil tuple (list, is_cache) agar konsisten dengan --search
                    let (list, is_cache) = execute_with_spinner(
                        "Memvalidasi distro...", 
                        get_lxc_distro_list()
                    ).await;

                    // Opsional: Kasih tahu user kalau validasinya pakai data cache
                    if is_cache {
                        println!("{}[INFO]{} Memvalidasi kode '{}' menggunakan data lokal.", YELLOW, RESET, code);
                    }

                    // Cari metadata distro berdasarkan slug (kode unik)
                    if let Some(meta) = list.into_iter().find(|d| d.slug == *code) {
                        // Jalankan fungsi create_new_container yang sudah async
                        execute_with_spinner(
                            &format!("Sedang membuat container {}...", name), 
                            create_new_container(name, meta)
                        ).await;
                        
                        println!("{}[SUCCESS]{} Container berhasil dibuat!", GREEN, RESET);
                    } else {
                        println!("{}[ERROR]{} Kode '{}' tidak ditemukan di daftar distro.", RED, code, RESET);
                        println!("{}Tip:{} Gunakan 'melisa --search' untuk melihat daftar kode yang tersedia.", YELLOW, RESET);
                    }
                },
                "--delete" => {
                    if let Some(name) = parts.get(2) {
                        // Import flush standar di sini agar lebih galak
                        use std::io::{self as std_io, Write};

                        println!("{}[INFO]{} Memvalidasi penghapusan untuk '{}'...", YELLOW, RESET, name);

                        // 1. Cetak prompt
                        print!("{}Are you sure delete '{}'? {} (y/N): {}", BOLD, name, RED, RESET);
                        
                        // 2. PAKSA FLUSH (Pake std::io agar instan muncul di terminal)
                        std_io::stdout().flush().expect("Gagal flush stdout");

                        let mut confirmation = String::new();
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin);
                        
                        // 3. Baca input
                        if let Ok(_) = reader.read_line(&mut confirmation).await {
                            let input = confirmation.trim().to_lowercase();
                            
                            // Jika user cuma pencet Enter, jangan biarkan lanjut
                            if input.is_empty() {
                                println!("{}[CANCEL]{} Tidak ada input, penghapusan dibatalkan.", YELLOW, RESET);
                                return ExecResult::Continue;
                            }

                            if input == "y" || input == "yes" {
                                // 4. Panggil dengan spinner
                                execute_with_spinner(
                                    &format!("sedang menghapus container {}", name),
                                    delete_container(name)
                                ).await;
                            } else {
                                println!("{}[CANCEL]{} Penghapusan dibatalkan.", YELLOW, RESET);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Nama container harus diisi. Contoh: melisa --delete mybox", RED, RESET);
                    }
                },
                // 2. Tambahkan .await pada pemanggilan Command luar (jika ada)
                "--run" => {
                    if let Some(name) = parts.get(2) {
                        start_container(name).await;
                    } else {
                        // Tambahkan feedback jika nama kosong
                        println!("{}[ERROR]{} Nama container harus diisi! Contoh: melisa --run mybox", RED, RESET);
                    }
                },
                "--use" => {
                    if let Some(name) = parts.get(2) {
                        attach_to_container(name).await;
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --use <name>{}", RED, RESET);
                    }
                }, 
                "--share" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        add_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}Usage: melisa --share <name> <host_path> <container_path>{}", RED, RESET);
                    }
                },
                "--reshare" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        remove_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}Usage: melisa --reshare <name> <host_path> <container_path>{}", RED, RESET);
                    }
                },
                "--send" => {
                    if let Some(name) = parts.get(2) {
                        // Ambil semua argumen mulai dari indeks ke-3 sampai habis
                        let cmd_to_send = &parts[3..]; 
                        
                        if !cmd_to_send.is_empty() {
                            send_command(name, cmd_to_send).await;
                        } else {
                            println!("{}Usage: melisa --send <name> <command>{}", RED, RESET);
                            println!("Example: melisa --send mybox apt update");
                        }
                    } else {
                        println!("{}Error: Name required.{}", RED, RESET);
                    }
                },
                "--upload" => {
                    if let (Some(name), Some(dest)) = (parts.get(2), parts.get(3)) {
                        upload_to_container(name, dest).await;
                    } else {
                        println!("{}Usage: melisa --upload <name> <dest_path>{}", RED, RESET);
                    }
                },
                "--list" => {
                    list_containers(false).await;
                },
                "--active" => {
                    list_containers(true).await;
                },
                "--stop" => {
                    if let Some(name) = parts.get(2) {
                        stop_container(name).await;
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --stop <name>{}", RED, RESET);
                    }
                },
                "--add" => {
                    if let Some(name) = parts.get(2) {
                        add_melisa_user(name).await;
                    } else {
                        println!("{}Usage: melisa --add <username>{}", RED, RESET);
                    }
                },
                "--passwd" => {
                    if let Some(name) = parts.get(2) {
                        set_user_password(name).await;
                    } else {
                        println!("{}Usage: melisa --passwd <username>{}", RED, RESET);
                    }
                },
                "--remove" => {
                    if let Some(name) = parts.get(2) {
                        println!("{}Are you sure delete user '{}'? (y/N){}", YELLOW, name, RESET);
                        let _ = io::stdout().flush().await; // Flush agar text muncul

                        let mut conf = String::new();
                        let mut reader = io::BufReader::new(io::stdin());
                        if reader.read_line(&mut conf).await.is_ok() {
                            if conf.trim().to_lowercase() == "y" {
                                // Pastikan fungsi ini juga async!
                                delete_melisa_user(name).await; 
                            }
                        }
                    } else {
                        println!("{}Usage: melisa --del <user>{}", RED, RESET);
                    }
                },
                "--user" => {
                    list_melisa_users().await;
                },
                "--upgrade" => {
                    if let Some(name) = parts.get(2) {
                        upgrade_user(name).await;
                    } else {
                        println!("{}Usage: melisa --upgrade <username>{}", RED, RESET);
                    }
                },
                "--clean" => {
                    clean_orphaned_sudoers().await;
                },
                "--new_project" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Hanya Admin yang bisa membuat project baru!", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if let Some(project_name) = parts.get(2) {
                        new_project(project_name).await;
                    } else {
                        println!("Usage: melisa --new_project <project_name>");
                    }
                },        
                "--invite" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Hanya Admin yang bisa mengundang user!", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("Usage: melisa --invite <project_name> <user1> <user2> ...");
                        return ExecResult::Continue;
                    }

                    let project_name = parts[2];
                    let invited_users = &parts[3..];
                    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

                    // Cek apakah master project ada
                    if !std::path::Path::new(&master_path).exists() {
                        println!("{}[ERROR]{} Master Project '{}' tidak ditemukan!", RED, RESET, project_name);
                        return ExecResult::Continue;
                    }

                    invite(project_name, invited_users).await;
                },
                "--pull" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Hanya Admin yang bisa melakukan pull!", RED, RESET);
                        return ExecResult::Continue;
                    }
                    if parts.len() < 3 {
                        println!("Usage: melisa --pull <from_user> <project_name>");
                        return ExecResult::Continue;
                    }
                    let project_name = parts[2];
                    let from_user = parts[3];

                    pull(from_user, project_name).await;

                },
                "--projects" => {
                    list_projects(home).await;
                },
                "--delete_project" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Hanya Admin yang bisa menghapus project!", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if let Some(project_name) = parts.get(2) {
                        let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

                        // 1. Cek apakah master project ada
                        if !std::path::Path::new(&master_path).exists() {
                            println!("{}[ERROR]{} Master Project '{}' tidak ditemukan!", RED, RESET, project_name);
                            return ExecResult::Continue;
                        }
                        delete_project(master_path, project_name).await;
                    }

                        
                },

                "--out" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Hanya Admin yang bisa mengeluarkan user!", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("Usage: melisa --out <project_name> <user1> <user2> ...");
                        return ExecResult::Continue;
                    }

                    let project_name = parts[2];
                    let targets = &parts[3..];

                    out_user(targets, project_name).await;
                },

                "--update" => {
                    let username = &parts[2];
                    let project_name = &parts[3];
                    let mode = parts.contains(&"--force");

                    if parts.len() < 2 {
                        println!("nama , nama project dimana")
                    }
                    update_project(username, project_name, mode).await;
                }
                "" => {
                    println!("{}Usage: melisa [options]{}", RED, RESET);
                    println!("Try 'melisa --help' for more information.");
                },
                _ => {
                    println!("{}melisa: unknown option '{}'{}", RED, sub_cmd, RESET);
                }
            }
            ExecResult::Continue
        },

        "exit" | "quit" => {
            println!("{BOLD}[melisa] Bay Bay...{RESET}");
            ExecResult::Break
        },

        "cd" => {
            let target = parts.get(1).map(|&s| if s == "~" { home } else { s }).unwrap_or(home);
            if let Err(e) = env::set_current_dir(target) {
                ExecResult::Error(format!("{}cd: {}{}", RED, e, RESET))
            } else {
                ExecResult::Continue
            }
        },

        // 3. Eksekusi Perintah System Secara Async
        _ => {
            let cargo_bin = format!("{}/.cargo/bin", home);
            let path_env = format!("{}:{}", cargo_bin, env::var("PATH").unwrap_or_default());

            // Menggunakan tokio::process::Command
            let status = Command::new("bash")
                .env("PATH", path_env)
                .env("HOME", home)
                .env("USER", user)
                .envs([
                    ("RUSTUP_HOME", format!("{}/.rustup", home)),
                    ("CARGO_HOME", format!("{}/.cargo", home)),
                    ("RUSTUP_TOOLCHAIN", "stable".into())
                ])
                .args(["-c", input])
                .status()
                .await; // <--- WAJIB AWAIT
            
            match status {
                Ok(_) => ExecResult::Continue,
                Err(e) => ExecResult::Error(format!("Execution error: {}", e)),
            }
        }
    }
}