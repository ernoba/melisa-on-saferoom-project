use std::{env, process::Command};
use std::io::{self, Write};

use crate::cli::container::create_new_container;
use crate::cli::container::delete_container;
use crate::cli::container::start_container;
use crate::cli::container::attach_to_container;
use crate::cli::container::stop_container;
use crate::cli::container::send_command;
use crate::cli::container::list_containers;
use crate::cli::color_text::{RED, BOLD, RESET};

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
                    println!("{}Usage: melisa [options]{}", BOLD, RESET);
                    println!("Options:");
                    println!("  --help             Show this help message");
                    println!("  --create <name>    Create a new LXC container");
                    println!("  --delete <name>    Delete an existing LXC container");
                    println!("  --run <name>       Run a command inside a container");
                    println!("  --use <name>       Attach to a container interactively");
                    println!("  --stop <name>      Stop a running container");
                    println!("  --list             List all containers");
                    println!("  --active           List only active (running) containers");
                },
                "--create" => {
                    if let Some(name) = parts.get(2) {
                        create_new_container(name);
                    } else {
                        println!("{}Error: Container name is required. Usage: melisa --create <name>{}", RED, RESET);
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