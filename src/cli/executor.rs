use tokio::process::Command; // Utilize Tokio for non-blocking asynchronous process execution
use tokio::io::{self, AsyncBufReadExt}; // For async stdin reading
use std::env;
use std::io::Write; // Required to forcefully flush stdout for interactive prompts

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
use crate::core::project_management::{
    PROJECTS_MASTER, delete_project, invite, list_projects, 
    new_project, out_user, pull, update_project, update_all_users
};
use crate::core::metadata::{print_version, inspect_container_metadata, MelisaError};

/// Defines the execution state after a command is processed
pub enum ExecResult {
    Continue,      // Proceed to the next command loop
    Break,         // Terminate the shell session
    ResetHistory,  // Signal the shell to purge command history
    Error(String), // Return a formatted error message
}

/// Core asynchronous command router. Parses raw string input and dispatches 
/// execution to the appropriate subsystem or system binary.
pub async fn execute_command(input: &str, user: &str, home: &str) -> ExecResult {
    // Tokenize the input string by whitespace into a vector of string slices
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() { return ExecResult::Continue; }

    // Match the primary command trigger
    match parts[0] {
        "melisa" => {
            // Safely extract the subcommand, defaulting to an empty string if missing
            let sub_cmd = parts.get(1).copied().unwrap_or("");

            match sub_cmd {
                "--help" | "-h" => {
                    let is_admin = admin_check().await;
                    
                    println!("\n{}MELISA CONTROL INTERFACE - VERSION 0.1.2{}", BOLD, RESET);
                    println!("Usage: melisa [options]\n");

                    println!("{}GENERAL COMMANDS{}", BOLD, RESET);
                    println!("  --help, -h             Display this comprehensive help manual");
                    println!("  --version              Display system version and project metadata");
                    println!("  --projects             List all projects associated with your workspace");
                    println!("  --update <project>     Synchronize project workdir via force-reset (overwrites local)");
                    println!("  --list                 Enumerate all LXC containers provisioned to your UID");
                    println!("  --active               Filter and display only running LXC containers");
                    println!("  --run <name>           Initiate the startup sequence for a specific container");
                    println!("  --stop <name>          Gracefully terminate a running container session");
                    println!("  --use <name>           Execute an interactive TTY session (shell attach)");
                    println!("  --send <name> <cmd>    Dispatch a non-interactive command directly to a container");
                    println!("  --info <name>          Retrieve and display container metadata");
                    println!("  --upload <name> <dest> Upload local artifacts to a container destination path");

                    if is_admin {
                        println!("\n{}ADMINISTRATION & INFRASTRUCTURE{}", BOLD, RESET);
                        println!("  --setup                Execute host-level environment initialization");
                        println!("  --clear                Purge the internal command history buffer");
                        println!("  --clean                Prune orphaned sudoers configurations");
                        println!("  --search <keyword>     Query remote repositories for validated LXC distributions");
                        println!("  --create <name> <code> Provision a new container from a specific distribution code");
                        println!("  --delete <name>        Decommission and destroy a container (requires confirmation)");
                        println!("  --share <n> <h> <c>    Mount a host directory into a container namespace");
                        println!("  --reshare <n> <h> <c>  Unmount a host directory from a container namespace");

                        println!("\n{}IDENTITY & ACCESS MANAGEMENT{}", BOLD, RESET);
                        println!("  --user                 Enumerate all registered Melisa system identities");
                        println!("  --add <user>           Provision a new user with Melisa-restricted shell");
                        println!("  --remove <user>        De-provision a user and revoke system permissions");
                        println!("  --upgrade <user>       Elevate a user to administrative privileges");
                        println!("  --passwd <user>        Update the authentication credentials for a user");

                        println!("\n{}PROJECT ORCHESTRATION{}", BOLD, RESET);
                        println!("  --new_project <name>   Initialize a new master git repository in root storage");
                        println!("  --delete_project <n>   Decommission a project and purge all associated user workdirs");
                        println!("  --invite <p> <u..>     Grant project access to specified system users");
                        println!("  --out <p> <u..>        Revoke project access from specified system users");
                        println!("  --pull <user> <proj>   Merge and synchronize code from a user workdir to master");
                        println!("  --update-all <proj>    Force-propagate master updates to all project members");
                    }
                    println!("\n{}Note: System-level modifications require appropriate SUID elevation.{}", BOLD, RESET);
                },
                "--setup" => {
                    install().await;
                },
                "--clear" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} You do not have sufficient privileges to clear system history.", RED, RESET);
                        return ExecResult::Continue;
                    }
                    return ExecResult::ResetHistory
                },
                "--version" => {
                    print_version().await;
                },
                "--search" => {
                    let keyword = parts.get(2).unwrap_or(&"").to_lowercase();
                    
                    // Retrieve tuple (data, is_cache) with a visual loading spinner
                    // Karena fungsi ini mungkin tidak log pesan internal, kita biarkan _pb
                    let (list, is_cache) = execute_with_spinner(
                        "Synchronizing distribution list...", 
                        |_pb| get_lxc_distro_list()
                    ).await;

                    // SAFETY CHECK: Handle the scenario where LXC data retrieval completely fails
                    if list.is_empty() {
                        println!("{}[ERROR]{} Failed to retrieve the distribution list from LXC.", RED, RESET);
                        println!("{}Tip:{} Ensure LXC is properly configured and the network is reachable.", YELLOW, RESET);
                        return ExecResult::Continue; 
                    }

                    // Inform the user about the data source (Live vs Cache)
                    if is_cache {
                        println!("{}[CACHE]{} Displaying local data (Offline/Cached Mode).", YELLOW, RESET);
                    } else {
                        println!("{}[FRESH]{} Successfully synchronized the latest distribution index from the server.", GREEN, RESET);
                    }

                    println!("\n{:<20} | {:<10} | {:<10}", "UNIQUE CODE", "DISTRO", "ARCH");
                    println!("{}", "-".repeat(45)); // Visual separator line

                    // Filter and display results based on the optional keyword
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
                        println!("{}[ERROR]{} Container Name and Distribution Code are required!", RED, RESET);
                        println!("Usage: melisa --create <container_name> <distro_code>");
                        return ExecResult::Continue;
                    }

                    // Retrieve distribution metadata
                    let (list, is_cache) = execute_with_spinner(
                        "Validating distribution metadata...", 
                        |_pb| get_lxc_distro_list()
                    ).await;

                    // SAFETY CHECK: Prevent processing if the distribution list failed to load
                    if list.is_empty() {
                        println!("{}[ERROR]{} Failed to retrieve the distribution list. Cannot validate the code.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if is_cache {
                        println!("{}[INFO]{} Validating distribution code '{}' against local cache.", YELLOW, RESET, code);
                    }

                    // Locate the exact distribution metadata matching the user's slug/code
                    if let Some(meta) = list.into_iter().find(|d| d.slug == *code) {
                        // Execute container creation asynchronously
                        // AKTIFKAN pb di sini dan lemparkan ke create_new_container
                        execute_with_spinner(
                            &format!("Provisioning container '{}'...", name), 
                            |pb| create_new_container(name, meta, pb)
                        ).await;
                    } else {
                        println!("{}[ERROR]{} Code '{}' was not found in the distribution registry.", RED, code, RESET);
                        println!("{}Tip:{} Execute 'melisa --search' to view available distribution codes.", YELLOW, RESET);
                    }
                },
                "--delete" => {
                    if let Some(name) = parts.get(2) {
                        println!("{}[INFO]{} Validating deletion request for '{}'...", YELLOW, RESET, name);

                        // 1. Print interactive confirmation prompt
                        print!("{}Are you sure you want to permanently delete '{}'? (y/N): {}", RED, name, RESET);
                        
                        // 2. FORCE FLUSH: Ensure the prompt appears immediately before blocking for input
                        std::io::stdout().flush().expect("Failed to flush stdout");

                        let mut confirmation = String::new();
                        // [CRITICAL FIX]: Upgraded to tokio::io::stdin() for asynchronous reading
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin);
                        
                        // 3. Await user input
                        if let Ok(_) = reader.read_line(&mut confirmation).await {
                            let input = confirmation.trim().to_lowercase();
                            
                            // Abort if the user simply pressed Enter (Default: No)
                            if input.is_empty() {
                                println!("{}[CANCEL]{} No input detected. Deletion aborted.", YELLOW, RESET);
                                return ExecResult::Continue;
                            }

                            if input == "y" || input == "yes" {
                                // AKTIFKAN pb di sini dan lemparkan ke delete_container
                                execute_with_spinner(
                                    &format!("Destroying container '{}'...", name),
                                    |pb| delete_container(name, pb)
                                ).await;
                            } else {
                                println!("{}[CANCEL]{} Deletion sequence aborted.", YELLOW, RESET);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --delete <name>", RED, RESET);
                    }
                },
                "--run" => {
                    if let Some(name) = parts.get(2) {
                        start_container(name).await;
                    } else {
                        println!("{}[ERROR]{} Container name is required! Usage: melisa --run <name>", RED, RESET);
                    }
                },
                "--use" => {
                    if let Some(name) = parts.get(2) {
                        attach_to_container(name).await;
                    } else {
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --use <name>{}", RED, BOLD, RESET);
                    }
                }, 
                "--share" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        add_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --share <name> <host_path> <container_path>{}", RED, BOLD, RESET);
                    }
                },
                "--reshare" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) = (parts.get(2), parts.get(3), parts.get(4)) {
                        remove_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --reshare <name> <host_path> <container_path>{}", RED, BOLD, RESET);
                    }
                },
                "--send" => {
                    if let Some(name) = parts.get(2) {
                        // Extract all subsequent arguments as the command payload
                        let cmd_to_send = &parts[3..]; 
                        
                        if !cmd_to_send.is_empty() {
                            send_command(name, cmd_to_send).await;
                        } else {
                            println!("{}[ERROR]{} Usage: melisa --send <name> <command>{}", RED, BOLD, RESET);
                            println!("Example: melisa --send mybox apt update");
                        }
                    } else {
                        println!("{}[ERROR]{} Container name required.{}", RED, BOLD, RESET);
                    }
                },
                "--info" => {
                    if let Some(name) = parts.get(2) {
                        println!("{}Searching metadata for container: {}...{}", BOLD, name, RESET);

                        match inspect_container_metadata(name).await {
                            Ok(data) => {
                                println!("\n--- [ MELISA CONTAINER INFO ] ---");
                                println!("{}", data.trim());
                                println!("----------------------------------");
                            },
                            Err(MelisaError::MetadataNotFound(_)) => {
                                println!("{}[ERROR]{} Container '{}' lacks MELISA metadata. It may not have been provisioned via the MELISA Engine.", RED, RESET, name);
                            },
                            Err(e) => {
                                println!("{}[ERROR]{} An unexpected error occurred: {}", RED, RESET, e);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --info <name>{}", RED, BOLD, RESET);
                    }
                },
                "--upload" => {
                    if let (Some(name), Some(dest)) = (parts.get(2), parts.get(3)) {
                        upload_to_container(name, dest).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --upload <name> <dest_path>{}", RED, BOLD, RESET);
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
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --stop <name>{}", RED, BOLD, RESET);
                    }
                },
                "--add" => {
                    if let Some(name) = parts.get(2) {
                        add_melisa_user(name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --add <username>{}", RED, BOLD, RESET);
                    }
                },
                "--passwd" => {
                    if let Some(name) = parts.get(2) {
                        set_user_password(name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --passwd <username>{}", RED, BOLD, RESET);
                    }
                },
                "--remove" => {
                    if let Some(name) = parts.get(2) {
                        print!("{}Are you sure you want to permanently delete user '{}'? (y/N): {}", RED, name, RESET);
                        std::io::stdout().flush().expect("Failed to flush stdout"); 

                        let mut conf = String::new();
                        // [CRITICAL FIX]: Upgraded to tokio::io::stdin() for asynchronous reading
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin);
                        
                        if let Ok(_) = reader.read_line(&mut conf).await {
                            let input = conf.trim().to_lowercase();
                            
                            if input.is_empty() {
                                println!("{}[CANCEL]{} No input detected. User deletion aborted.", YELLOW, RESET);
                                return ExecResult::Continue;
                            }

                            if input == "y" || input == "yes" {
                                delete_melisa_user(name).await; 
                            } else {
                                println!("{}[CANCEL]{} User deletion aborted.", YELLOW, RESET);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --remove <username>{}", RED, BOLD, RESET);
                    }
                },
                "--user" => {
                    list_melisa_users().await;
                },
                "--upgrade" => {
                    if let Some(name) = parts.get(2) {
                        upgrade_user(name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --upgrade <username>{}", RED, BOLD, RESET);
                    }
                },
                "--clean" => {
                    clean_orphaned_sudoers().await;
                },
                "--new_project" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can provision new master projects.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if let Some(project_name) = parts.get(2) {
                        new_project(project_name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --new_project <project_name>{}", RED, BOLD, RESET);
                    }
                },        
                "--invite" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can assign users to projects.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("{}[ERROR]{} Usage: melisa --invite <project_name> <user1> <user2> ...{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }

                    let project_name = parts[2];
                    let invited_users = &parts[3..];
                    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

                    // Validate master project existence
                    if !std::path::Path::new(&master_path).exists() {
                        println!("{}[ERROR]{} Master Project '{}' does not exist.", RED, RESET, project_name);
                        return ExecResult::Continue;
                    }

                    invite(project_name, invited_users).await;
                },
                "--pull" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can execute a forced pull.", RED, RESET);
                        return ExecResult::Continue;
                    }
                    if parts.len() < 3 {
                        println!("{}[ERROR]{} Usage: melisa --pull <from_user> <project_name>{}", RED, BOLD, RESET);
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
                        println!("{}[ERROR]{} Only Administrators can delete master projects.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if let Some(project_name) = parts.get(2) {
                        let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

                        if !std::path::Path::new(&master_path).exists() {
                            println!("{}[ERROR]{} Master Project '{}' does not exist.", RED, RESET, project_name);
                            return ExecResult::Continue;
                        }
                        delete_project(master_path, project_name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --delete_project <project_name>{}", RED, BOLD, RESET);
                    }
                },
                "--out" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can revoke user access.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("{}[ERROR]{} Usage: melisa --out <project_name> <user1> <user2> ...{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }

                    let project_name = parts[2];
                    let targets = &parts[3..];

                    out_user(targets, project_name).await;
                },
                "--update" => {
                    if parts.len() < 3 {
                        println!("{}[ERROR]{} Usage: melisa --update <project_name> [--force]", RED, RESET);
                        return ExecResult::Continue;
                    }

                    // Flag extraction: check if --force is present anywhere in the argument list
                    let force_mode = parts.contains(&"--force");

                    // Filter out flags and base commands to isolate the user and project targets
                    let clean_args: Vec<&str> = parts.iter()
                        .filter(|&&x| x != "--force" && x != "--update" && x != "melisa")
                        .copied()
                        .collect();

                    // If clean_args length is 1, default to the current executing user
                    // If clean_args length >= 2, assume [0] is the target user and [1] is the project
                    let (target_user, project_name) = if clean_args.len() == 1 {
                        (user, clean_args[0])
                    } else if clean_args.len() >= 2 {
                        (clean_args[0], clean_args[1])
                    } else {
                        println!("{}[ERROR]{} Invalid argument structure.", RED, RESET);
                        return ExecResult::Continue;
                    };

                    update_project(target_user, project_name, force_mode).await;
                },
                "--update-all" => {
                    if parts.len() < 3 {
                        println!("{}[ERROR]{} Usage: melisa --update-all <project_name>{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }
                    let project_name = parts[2];
                    update_all_users(project_name).await;
                }
                "" => {
                    println!("{}[ERROR]{} Incomplete command. Usage: melisa [options]", RED, RESET);
                    println!("Execute 'melisa --help' for a detailed list of available commands.");
                },
                _ => {
                    println!("{}[ERROR]{} Unknown option '{}'", RED, RESET, sub_cmd);
                    println!("Execute 'melisa --help' to view the manual.");
                }
            }
            ExecResult::Continue
        },

        "exit" | "quit" => {
            println!("{}[SYSTEM]{} Terminating secure session... Goodbye.{}", BOLD, YELLOW, RESET);
            ExecResult::Break
        },

        "cd" => {
            // Note: env::set_current_dir modifies the process state globally. 
            // In a multi-threaded async app, this affects all tasks, but for a CLI shell simulator, it's expected.
            let target = parts.get(1).copied().unwrap_or(home);
            let target = if target == "~" { home } else { target };

            if let Err(e) = env::set_current_dir(target) {
                ExecResult::Error(format!("{}cd: {}{}", RED, e, RESET))
            } else {
                ExecResult::Continue
            }
        },

        // Fallback: Dispatch unrecognized commands directly to the Host system's Bash shell
        _ => {
            let cargo_bin = format!("{}/.cargo/bin", home);
            let path_env = format!("{}:{}", cargo_bin, env::var("PATH").unwrap_or_default());

            // Utilize Tokio Command to spawn the process asynchronously without blocking the executor
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
                .await; // <-- REQUIRED AWAIT to prevent zombie processes
            
            match status {
                Ok(_) => ExecResult::Continue,
                Err(e) => ExecResult::Error(format!("Bash Execution Error: {}", e)),
            }
        }
    }
}