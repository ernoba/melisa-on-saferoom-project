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

pub enum ExecResult {
    Continue,
    Break,
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
                    if !admin_check() {
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
                        println!("  --clean            Clean orphaned sudoers files for non-existent users");
                        println!("  --upload <name> <dest_path>  Upload a file to a container");
                        println!("  --share <name> <host_path> <cont_path>  Share a folder between host and container");
                        println!("  --reshare <name> <host_path> <cont_path>  Remove a shared folder between host and container");
                     }
                },
                "--setup" => {
                    // Jika install() melakukan operasi I/O berat, 
                    // pertimbangkan menjadikannya async juga
                    install().await;
                },
                "--search" => {
                    let keyword = parts.get(2).unwrap_or(&"").to_lowercase();
                    let list = execute_with_spinner("Sedang mencari distro...", || {
                        get_lxc_distro_list()
                    });

                    println!("{:<20} | {:<10} | {:<10}", "KODE UNIK", "DISTRO", "ARCH");
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

                    let list = execute_with_spinner("Memvalidasi distro...", || {
                        get_lxc_distro_list()
                    });

                    if let Some(meta) = list.into_iter().find(|d| d.slug == *code) {
                        // Container creation biasanya lama, pastikan create_new_container async
                        execute_with_spinner(&format!("Sedang membuat container {}...", name), || {
                            create_new_container(name, meta);
                        });
                        println!("{}[SUCCESS]{} Container berhasil dibuat!", GREEN, RESET);
                    } else {
                        println!("{}[ERROR]{} Kode '{}' tidak ditemukan.", RED, code, RESET);
                    }
                },
                "--delete" => {
                    if let Some(name) = parts.get(2) {
                        print!("{}Are you sure delete '{}'? {} (y/N) {}", BOLD, name, RED, RESET);
                        let _ = io::stdout().flush().await; // Async flush

                        let mut confirmation = String::new();
                        let mut reader = io::BufReader::new(io::stdin());
                        if reader.read_line(&mut confirmation).await.is_ok() {
                            if confirmation.trim().eq_ignore_ascii_case("y") {
                                delete_container(name);
                            }
                        }
                    }
                },
                // 2. Tambahkan .await pada pemanggilan Command luar (jika ada)
                "--run" => {
                    if let Some(name) = parts.get(2) {
                        start_container(name); // Jika ini memanggil LXC, jadikan async
                    }
                },
                "--use" => {
                    if let Some(name) = parts.get(2) {
                        attach_to_container(name);
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --use <name>{}", RED, RESET);
                    }
                }, 
                "--share" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        add_shared_folder(name, host_p, cont_p);
                    } else {
                        println!("{}Usage: melisa --share <name> <host_path> <container_path>{}", RED, RESET);
                    }
                },
                "--reshare" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        remove_shared_folder(name, host_p, cont_p);
                    } else {
                        println!("{}Usage: melisa --reshare <name> <host_path> <container_path>{}", RED, RESET);
                    }
                },
                "--send" => {
                    if let Some(name) = parts.get(2) {
                        // Ambil semua argumen mulai dari indeks ke-3 sampai habis
                        let cmd_to_send = &parts[3..]; 
                        
                        if !cmd_to_send.is_empty() {
                            send_command(name, cmd_to_send);
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
                        upload_to_container(name, dest);
                    } else {
                        println!("{}Usage: melisa --upload <name> <dest_path>{}", RED, RESET);
                    }
                },
                "--list" => {
                    list_containers(false);
                },
                "--active" => {
                    list_containers(true);
                },
                "--stop" => {
                    if let Some(name) = parts.get(2) {
                        stop_container(name);
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --stop <name>{}", RED, RESET);
                    }
                },
                "--add" => {
                    if let Some(name) = parts.get(2) {
                        add_melisa_user(name);
                    } else {
                        println!("{}Usage: melisa --add <username>{}", RED, RESET);
                    }
                },
                "--passwd" => {
                    if let Some(name) = parts.get(2) {
                        set_user_password(name);
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
                                delete_melisa_user(name); 
                            }
                        }
                    } else {
                        println!("{}Usage: melisa --del <user>{}", RED, RESET);
                    }
                },
                "--user" => {
                    list_melisa_users();
                },
                "--upgrade" => {
                    if let Some(name) = parts.get(2) {
                        upgrade_user(name);
                    } else {
                        println!("{}Usage: melisa --upgrade <username>{}", RED, RESET);
                    }
                },
                "--clean" => {
                    clean_orphaned_sudoers();
                },        
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