use std::{env, process::Command};
use std::io::{self, Write};

use indicatif::{ProgressBar, ProgressStyle}; // Import library
use std::time::Duration;

use crate::core::container::*;
use crate::core::setup::install;
use crate::core::root_check::admin_check;
use crate::distros::distro::get_lxc_distro_list;
use crate::cli::color_text::{RED,YELLOW, BOLD, RESET};
use crate::core::user_management::{add_melisa_user,set_user_password, delete_melisa_user, list_melisa_users, upgrade_user, clean_orphaned_sudoers};

pub enum ExecResult {
    Continue,
    Break,
    Error(String),
}

pub fn execute_command(input: &str, user: &str, home: &str) -> ExecResult {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() { return ExecResult::Continue; }

    match parts[0] {
        // MAIN COMMANDS is melisa
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
                        println!("  --create <name>    Create a new LXC container");
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
                    install();
                },
                "--search" => {
                    let keyword = parts.get(2).unwrap_or(&"").to_lowercase();

                    // 1. Inisialisasi Spinner
                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                            .template("{spinner:.green} {msg}")
                            .unwrap(),
                    );
                    pb.set_message("Sedang mencari distro...");
                    pb.enable_steady_tick(Duration::from_millis(100)); // Jalankan animasi setiap 100ms

                    // 2. Jalankan fungsi yang berat/lambat
                    let list = get_lxc_distro_list();

                    // 3. Berhentikan dan hapus spinner sebelum mencetak hasil
                    pb.finish_and_clear();

                    // 4. Cetak hasil seperti biasa
                    println!("{:<20} | {:<10} | {:<10}", "KODE UNIK", "DISTRO", "ARCH");
                    for d in list {
                        if d.slug.contains(&keyword) || d.name.contains(&keyword) {
                            println!("{:<20} | {:<10} | {:<10}", d.slug, d.name, d.arch);
                        }
                    }
                },
                "--create" => {
                    let name = parts.get(2).expect("Nama container?");
                    let code = parts.get(3).expect("Kode unik distro?");
                    let list = get_lxc_distro_list();
                    if let Some(meta) = list.into_iter().find(|d| d.slug == *code) {
                        create_new_container(name, meta);
                    } else {
                        println!("Error: Kode '{}' tidak ada.", code);
                    }
                },
                "--delete" => {
                    if let Some(name) = parts.get(2) {
                        print!("{}Are you sure you want to delete container '{}'? {}This action cannot be undone. (y/N) {}",
                            BOLD, name, RED, RESET);
                        let _ = io::stdout().flush(); // WAJIB ADA
                        let mut confirmation = String::new();
                        if std::io::stdin().read_line(&mut confirmation).is_ok() {
                            if confirmation.trim().eq_ignore_ascii_case("y") {
                                delete_container(name);
                            }
                        }
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --delete-container <name>{}", RED, RESET);
                    }
                },
                "--run" => {
                    if let Some(name) = parts.get(2) {
                        start_container(name);
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --run <name>{}", RED, RESET);
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
                        let mut conf = String::new();
                        io::stdin().read_line(&mut conf).unwrap();
                        if conf.trim().to_lowercase() == "y" {
                            delete_melisa_user(name);
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

        _ => {
            let cargo_bin = format!("{}/.cargo/bin", home);
            let path_env = format!("{}:{}", cargo_bin, env::var("PATH").unwrap_or_default());

            let _ = Command::new("bash")
                .env("PATH", path_env)
                .env("HOME", home)
                .env("USER", user)
                .envs([
                    ("RUSTUP_HOME", format!("{}/.rustup", home)),
                    ("CARGO_HOME", format!("{}/.cargo", home)),
                    ("RUSTUP_TOOLCHAIN", "stable".into())
                ])
                .args(["-c", input])
                .status();
            
            ExecResult::Continue
        }
    }
}