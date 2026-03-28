use tokio::process::Command;
use tokio::io::{self, AsyncBufReadExt};
use std::env;
use std::io::Write;

use crate::core::container::*;
use crate::core::setup::install;
use crate::core::root_check::admin_check;
use crate::distros::distro::get_lxc_distro_list;
use crate::cli::loading::execute_with_spinner;
use crate::cli::color_text::{RED, GREEN, YELLOW, BOLD, RESET};
use crate::core::user_management::{
    add_melisa_user, set_user_password, delete_melisa_user,
    list_melisa_users, upgrade_user, clean_orphaned_sudoers,
};
use crate::core::project_management::{
    PROJECTS_MASTER, delete_project, invite, list_projects,
    new_project, out_user, pull, update_project, update_all_users,
};
use crate::core::metadata::{print_version, inspect_container_metadata, MelisaError};

/// Defines the execution state after a command is processed.
pub enum ExecResult {
    Continue,
    Break,
    ResetHistory,
    Error(String),
}

/// Core asynchronous command router.
///
/// # Flag `--audit`
/// Dapat disisipkan di MANA SAJA dalam perintah, contoh:
///   melisa --create mybox ubu-jammy --audit
///   melisa --audit --delete open-suse
///   melisa --stop mybox --audit
///
/// Ketika flag ini ada:
///   1. Spinner disembunyikan (ProgressBar::hidden) sehingga tidak ada jejak
///      timestamp di layar.
///   2. Output mentah dari subprocess (lxc-*, git, apt, userdel, dll.) diteruskan
///      langsung ke terminal via Stdio::inherit.
///   3. Pesan debug/internal yang biasanya disembunyikan juga ditampilkan.

pub fn parse_command(input: &str) -> (Vec<String>, bool) {
    let raw_parts: Vec<&str> = input.split_whitespace().collect();
    let audit = raw_parts.contains(&"--audit");
    let parts: Vec<String> = raw_parts
        .into_iter()
        .filter(|&x| x != "--audit")
        .map(String::from)
        .collect();
    (parts, audit)
}

pub async fn execute_command(input: &str, user: &str, home: &str) -> ExecResult {
    let (parts, audit) = parse_command(input);
    if parts.is_empty() {
        return ExecResult::Continue;
    }
    match parts[0].as_str() {
        "melisa" => {
            let sub_cmd = parts.get(1).map(|s| s.as_str()).unwrap_or("");
            match sub_cmd {
                "--help" | "-h" => {
                    let is_admin = admin_check().await;
                    println!("\n{}MELISA CONTROL INTERFACE - VERSION 0.1.3{}", BOLD, RESET);
                    println!("Usage: melisa [options] [--audit]\n");
                    println!("{}[--audit]{} can be added to any command to display hidden logs", YELLOW, RESET);
                    println!("and display subprocess output directly to the terminal.\n");
                    println!("{}GENERAL COMMANDS{}", BOLD, RESET);
                    println!("  --help, -h             Show help guide");
                    println!("  --version              Show system version");
                    println!("  --ip <n>               Get internal IP of the container");
                    println!("  --projects             List all workspace projects");
                    println!("  --update <project>     Synchronize project workdir via force-reset");
                    println!("  --list                 Show all LXC containers");
                    println!("  --active               Show only running containers");
                    println!("  --run <n>              Start container");
                    println!("  --stop <n>             Stop container");
                    println!("  --use <n>              Enter interactive container shell");
                    println!("  --send <n> <cmd>       Send command to container");
                    println!("  --info <n>             Show container metadata");
                    println!("  --upload <n> <dest>    Upload file to container");
                    println!("\n{}DEPLOYMENT ENGINE (.mel){}", BOLD, RESET);
                    println!("  --up <file.mel>        Deploy project from .mel manifest");
                    println!("  --down <file.mel>      Stop deployment from .mel manifest");
                    println!("  --mel-info <file.mel>  Show .mel manifest info");
                    if is_admin {
                        println!("\n{}ADMINISTRATION & INFRASTRUCTURE{}", BOLD, RESET);
                        println!("  --setup                Initialize host environment");
                        println!("  --clear                Clear command history");
                        println!("  --clean                Clean orphaned sudoers configuration");
                        println!("  --search <keyword>     Search available LXC distributions");
                        println!("  --create <n> <code>    Create new container from distribution code");
                        println!("  --delete <n>           Delete container (requires confirmation)");
                        println!("  --share <n> <h> <c>    Mount host directory to container");
                        println!("  --reshare <n> <h> <c>  Unmount directory from container");
                        println!("\n{}IDENTITY & ACCESS MANAGEMENT{}", BOLD, RESET);
                        println!("  --user                 List all Melisa identities");
                        println!("  --add <user>           Add new user");
                        println!("  --remove <user>        Remove user");
                        println!("  --upgrade <user>       Elevate user to admin");
                        println!("  --passwd <user>        Change user credentials");
                        println!("\n{}PROJECT ORCHESTRATION{}", BOLD, RESET);
                        println!("  --new_project <n>      Initialize new project repository");
                        println!("  --delete_project <n>   Delete project and all workdirs");
                        println!("  --invite <p> <u..>     Grant project access to user");
                        println!("  --out <p> <u..>        Revoke project access from user");
                        println!("  --pull <user> <proj>   Merge code from user workdir to master");
                        println!("  --update-all <proj>    Distribute master updates to all members");
                    }
                    println!("\n{}Note: System modifications require SUID elevation.{}", BOLD, RESET);
                }

                // ─── DEPLOYMENT ENGINE ───────────────────────────────────────
                "--up" => {
                    let mel_path = parts.get(2).map(|s| s.as_str()).unwrap_or("");
                    if mel_path.is_empty() {
                        println!("{}[ERROR]{} Path file .mel diperlukan.", RED, RESET);
                        println!("Usage: melisa --up <path/ke/program.mel>");
                        return ExecResult::Continue;
                    }
                    crate::deployment::deployer::cmd_up(mel_path, audit).await;
                }
                "--down" => {
                    let mel_path = parts.get(2).map(|s| s.as_str()).unwrap_or("");
                    if mel_path.is_empty() {
                        println!("{}[ERROR]{} Path file .mel diperlukan.", RED, RESET);
                        println!("Usage: melisa --down <path/ke/program.mel>");
                        return ExecResult::Continue;
                    }
                    crate::deployment::deployer::cmd_down(mel_path, audit).await;
                }
                "--mel-info" => {
                    let mel_path = parts.get(2).map(|s| s.as_str()).unwrap_or("");
                    if mel_path.is_empty() {
                        println!("{}[ERROR]{} Path file .mel diperlukan.", RED, RESET);
                        println!("Usage: melisa --mel-info <path/ke/program.mel>");
                        return ExecResult::Continue;
                    }
                    crate::deployment::deployer::cmd_mel_info(mel_path).await;
                }
                "--setup" => {
                    install().await;
                }

                "--clear" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} You do not have sufficient privileges to clear system history.", RED, RESET);
                        return ExecResult::Continue;
                    }
                    return ExecResult::ResetHistory;
                }

                "--version" => {
                    print_version().await;
                }

                "--search" => {
                    let keyword = parts.get(2).map(|s| s.as_str()).unwrap_or("").to_lowercase();

                    let (list, is_cache) = execute_with_spinner(
                        "Synchronizing distribution list...",
                        |_pb| get_lxc_distro_list(audit),
                        audit,
                    )
                    .await;

                    if list.is_empty() {
                        println!("{}[ERROR]{} Failed to retrieve the distribution list from LXC.", RED, RESET);
                        println!("{}Tip:{} Ensure LXC is properly configured and the network is reachable.", YELLOW, RESET);
                        return ExecResult::Continue;
                    }

                    if is_cache {
                        println!("{}[CACHE]{} Displaying local data (Offline/Cached Mode).", YELLOW, RESET);
                    } else {
                        println!("{}[FRESH]{} Successfully synchronized the latest distribution index from the server.", GREEN, RESET);
                    }

                    println!("\n{:<20} | {:<10} | {:<10}", "UNIQUE CODE", "DISTRO", "ARCH");
                    println!("{}", "-".repeat(45));

                    for d in list {
                        if d.slug.contains(&keyword) || d.name.contains(&keyword) {
                            println!("{:<20} | {:<10} | {:<10}", d.slug, d.name, d.arch);
                        }
                    }
                }

                "--create" => {
                    let name = parts.get(2).map(|s| s.as_str()).unwrap_or("");
                    let code = parts.get(3).map(|s| s.as_str()).unwrap_or("");

                    if name.is_empty() || code.is_empty() {
                        println!("{}[ERROR]{} Container Name and Distribution Code are required!", RED, RESET);
                        println!("Usage: melisa --create <container_name> <distro_code>");
                        return ExecResult::Continue;
                    }

                    let (list, is_cache) = execute_with_spinner(
                        "Validating distribution metadata...",
                        |_pb| get_lxc_distro_list(audit),
                        audit,
                    )
                    .await;

                    if list.is_empty() {
                        println!("{}[ERROR]{} Failed to retrieve the distribution list. Cannot validate the code.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if is_cache {
                        println!("{}[INFO]{} Validating distribution code '{}' against local cache.", YELLOW, RESET, code);
                    }

                    if let Some(meta) = list.into_iter().find(|d| d.slug == *code) {
                        execute_with_spinner(
                            &format!("Provisioning container '{}'...", name),
                            |pb| create_new_container(name, meta, pb, audit),
                            audit,
                        )
                        .await;
                    } else {
                        println!("{}[ERROR]{} Code '{}' was not found in the distribution registry.", RED, code, RESET);
                        println!("{}Tip:{} Execute 'melisa --search' to view available distribution codes.", YELLOW, RESET);
                    }
                }

                "--delete" => {
                    if let Some(name) = parts.get(2) {
                        println!("{}[INFO]{} Validating deletion request for '{}'...", YELLOW, RESET, name);

                        print!("{}Are you sure you want to permanently delete '{}'? (y/N): {}", RED, name, RESET);
                        std::io::stdout().flush().expect("Failed to flush stdout");

                        let mut confirmation = String::new();
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin);

                        if let Ok(_) = reader.read_line(&mut confirmation).await {
                            let input = confirmation.trim().to_lowercase();

                            if input.is_empty() {
                                println!("{}[CANCEL]{} No input detected. Deletion aborted.", YELLOW, RESET);
                                return ExecResult::Continue;
                            }

                            if input == "y" || input == "yes" {
                                execute_with_spinner(
                                    &format!("Destroying container '{}'...", name),
                                    |pb| delete_container(name, pb, audit),
                                    audit,
                                )
                                .await;
                            } else {
                                println!("{}[CANCEL]{} Deletion sequence aborted.", YELLOW, RESET);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --delete <n>", RED, RESET);
                    }
                }

                "--run" => {
                    if let Some(name) = parts.get(2) {
                        start_container(name, audit).await;
                    } else {
                        println!("{}[ERROR]{} Container name is required! Usage: melisa --run <n>", RED, RESET);
                    }
                }

                "--use" => {
                    if let Some(name) = parts.get(2) {
                        attach_to_container(name).await;
                    } else {
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --use <n>{}", RED, BOLD, RESET);
                    }
                }

                "--share" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) =
                        (parts.get(2), parts.get(3), parts.get(4))
                    {
                        add_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --share <n> <host_path> <container_path>{}", RED, BOLD, RESET);
                    }
                }

                "--reshare" => {
                    if let (Some(name), Some(host_p), Some(cont_p)) =
                        (parts.get(2), parts.get(3), parts.get(4))
                    {
                        remove_shared_folder(name, host_p, cont_p).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --reshare <n> <host_path> <container_path>{}", RED, BOLD, RESET);
                    }
                }

                "--send" => {
                    if let Some(name_raw) = parts.get(2) {
                        let name = name_raw.as_str(); // Ambil &str dari name
                        
                        // --- PROSES KONVERSI DI SINI ---
                        // Kita ambil slice dari index 3 sampai habis, 
                        // lalu ubah tiap String jadi &str (map), lalu kumpulkan (collect)
                        let cmd_to_send: Vec<&str> = parts[3..]
                            .iter()
                            .map(|s| s.as_str())
                            .collect();

                        if !cmd_to_send.is_empty() {
                            // Kirim referensi dari Vec yang baru kita buat
                            send_command(name, &cmd_to_send).await;
                        } else {
                            println!("{}[ERROR]{} Usage: melisa --send <n> <command>{}", RED, BOLD, RESET);
                            println!("Example: melisa --send mybox apt update");
                        }
                    } else {
                        println!("{}[ERROR]{} Container name required.{}", RED, BOLD, RESET);
                    }
                }

                "--info" => {
                    if let Some(name) = parts.get(2) {
                        println!("{}Searching metadata for container: {}...{}", BOLD, name, RESET);

                        match inspect_container_metadata(name).await {
                            Ok(data) => {
                                println!("\n--- [ MELISA CONTAINER INFO ] ---");
                                println!("{}", data.trim());
                                println!("----------------------------------");
                            }
                            Err(MelisaError::MetadataNotFound(_)) => {
                                println!(
                                    "{}[ERROR]{} Container '{}' lacks MELISA metadata. It may not have been provisioned via the MELISA Engine.",
                                    RED, RESET, name
                                );
                            }
                            Err(e) => {
                                println!("{}[ERROR]{} An unexpected error occurred: {}", RED, RESET, e);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --info <n>{}", RED, BOLD, RESET);
                    }
                }

                "--ip" => {
                    if let Some(name) = parts.get(2) {
                        match get_container_ip(name).await {
                            Some(ip) => println!("{}", ip),
                            None => {
                                eprintln!(
                                    "{}[ERROR]{} Cannot get IP for '{}'. Container may be stopped or lack DHCP.",
                                    RED, RESET, name
                                );
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --ip <container_name>{}", RED, BOLD, RESET);
                    }
                }

                "--upload" => {
                    if let (Some(name), Some(dest)) = (parts.get(2), parts.get(3)) {
                        upload_to_container(name, dest).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --upload <n> <dest_path>{}", RED, BOLD, RESET);
                    }
                }

                "--list" => {
                    list_containers(false).await;
                }

                "--active" => {
                    list_containers(true).await;
                }

                "--stop" => {
                    if let Some(name) = parts.get(2) {
                        stop_container(name, audit).await;
                    } else {
                        println!("{}[ERROR]{} Container name is required. Usage: melisa --stop <n>{}", RED, BOLD, RESET);
                    }
                }

                "--add" => {
                    if let Some(name) = parts.get(2) {
                        add_melisa_user(name, audit).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --add <username>{}", RED, BOLD, RESET);
                    }
                }

                "--passwd" => {
                    if let Some(name) = parts.get(2) {
                        set_user_password(name).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --passwd <username>{}", RED, BOLD, RESET);
                    }
                }

                "--remove" => {
                    if let Some(name) = parts.get(2) {
                        print!("{}Are you sure you want to permanently delete user '{}'? (y/N): {}", RED, name, RESET);
                        std::io::stdout().flush().expect("Failed to flush stdout");

                        let mut conf = String::new();
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin);

                        if let Ok(_) = reader.read_line(&mut conf).await {
                            let input = conf.trim().to_lowercase();

                            if input.is_empty() {
                                println!("{}[CANCEL]{} No input detected. User deletion aborted.", YELLOW, RESET);
                                return ExecResult::Continue;
                            }

                            if input == "y" || input == "yes" {
                                delete_melisa_user(name, audit).await;
                            } else {
                                println!("{}[CANCEL]{} User deletion aborted.", YELLOW, RESET);
                            }
                        }
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --remove <username>{}", RED, BOLD, RESET);
                    }
                }

                "--user" => {
                    list_melisa_users().await;
                }

                "--upgrade" => {
                    if let Some(name) = parts.get(2) {
                        upgrade_user(name, audit).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --upgrade <username>{}", RED, BOLD, RESET);
                    }
                }

                "--clean" => {
                    clean_orphaned_sudoers().await;
                }

                "--new_project" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can provision new master projects.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if let Some(project_name) = parts.get(2) {
                        new_project(project_name, audit).await;
                    } else {
                        println!("{}[ERROR]{} Usage: melisa --new_project <project_name>{}", RED, BOLD, RESET);
                    }
                }

                "--invite" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can assign users to projects.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("{}[ERROR]{} Usage: melisa --invite <project_name> <user1> <user2> ...{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }

                    // 1. Ambil sebagai referensi (&str), jangan dipindah (move)
                    let project_name = &parts[2]; 

                    // 2. Konversi slice &[String] menjadi Vec<&str> agar cocok dengan fungsi invite
                    let invited_users: Vec<&str> = parts[3..]
                        .iter()
                        .map(|s| s.as_str())
                        .collect();

                    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

                    if !std::path::Path::new(&master_path).exists() {
                        println!("{}[ERROR]{} Master Project '{}' does not exist.", RED, RESET, project_name);
                        return ExecResult::Continue;
                    }

                    // 3. Masukkan &invited_users (referensi ke Vec yang baru dibuat)
                    invite(project_name, &invited_users, audit).await;
                }

                "--pull" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can pull user workspaces.", RED, RESET);
                        return ExecResult::Continue;
                    }
                    if parts.len() < 4 {
                        println!("{}[ERROR]{} Usage: melisa --pull <from_user> <project_name>{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }

                    // TAMBAHKAN & DI SINI
                    let from_user = &parts[2]; 
                    let project_name = &parts[3];

                    // Sekarang variabel di atas bertipe &String, 
                    // yang otomatis bisa diterima oleh fungsi pull sebagai &str
                    let success = pull(from_user, project_name, audit).await;
                    
                    if !success {
                        return ExecResult::Continue;
                    }
                }

                "--projects" => {
                    list_projects(home).await;
                }

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
                }

                "--out" => {
                    if !admin_check().await {
                        println!("{}[ERROR]{} Only Administrators can revoke user access.", RED, RESET);
                        return ExecResult::Continue;
                    }

                    if parts.len() < 4 {
                        println!("{}[ERROR]{} Usage: melisa --out <project_name> <user1> <user2> ...{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }

                    // 1. Pinjam project_name, jangan di-move
                    let project_name = &parts[2];

                    // 2. Konversi slice &[String] menjadi Vec<&str>
                    let targets: Vec<&str> = parts[3..]
                        .iter()
                        .map(|s| s.as_str())
                        .collect();

                    // 3. Kirim referensi ke Vec targets dan project_name
                    out_user(&targets, project_name).await;
                }

                "--update" => {
                    if parts.len() < 3 {
                        println!("{}[ERROR]{} Usage: melisa --update <project_name> [--force]", RED, RESET);
                        return ExecResult::Continue;
                    }

                    // 1. Cara cek flag dalam Vec<String>
                    let force_mode = parts.iter().any(|s| s == "--force");

                    // 2. Filter argumen: Ubah dulu jadi &str baru di-filter
                    let clean_args: Vec<&str> = parts
                        .iter()
                        .map(|s| s.as_str()) // Kuncinya di sini: ubah String ke &str
                        .filter(|&x| x != "--force" && x != "--update" && x != "melisa")
                        .collect();

                    // 3. Ambil datanya (sekarang clean_args sudah berisi &str)
                    let (target_user, project_name) = if clean_args.len() == 1 {
                        (user, clean_args[0]) // user di sini adalah &str dari parameter fungsi
                    } else if clean_args.len() >= 2 {
                        (clean_args[0], clean_args[1])
                    } else {
                        println!("{}[ERROR]{} Invalid argument structure.", RED, RESET);
                        return ExecResult::Continue;
                    };

                    update_project(target_user, project_name, force_mode, audit).await;
                }

                "--update-all" => {
                    if parts.len() < 3 {
                        println!("{}[ERROR]{} Usage: melisa --update-all <project_name>{}", RED, BOLD, RESET);
                        return ExecResult::Continue;
                    }
                    let project_name = &parts[2];
                    update_all_users(project_name, audit).await;
                }

                "" => {
                    println!("{}[ERROR]{} Incomplete command. Usage: melisa [options]", RED, RESET);
                    println!("Execute 'melisa --help' for a detailed list of available commands.");
                }

                _ => {
                    println!("{}[ERROR]{} Unknown option '{}'", RED, RESET, sub_cmd);
                    println!("Execute 'melisa --help' to view the manual.");
                }
            }

            ExecResult::Continue
        }

        "exit" | "quit" => {
            println!("{}[SYSTEM]{} Terminating secure session... Goodbye.{}", BOLD, YELLOW, RESET);
            ExecResult::Break
        }

        "cd" => {
            // 1. Gunakan map(|s| s.as_str()) untuk mengubah Option<&String> menjadi Option<&str>
            let target = parts.get(1).map(|s| s.as_str()).unwrap_or(home);
            
            // 2. Logika shortcut home (~)
            let target = if target == "~" { home } else { target };

            // 3. Eksekusi perpindahan direktori
            if let Err(e) = env::set_current_dir(target) {
                ExecResult::Error(format!("{}cd: {}{}", RED, e, RESET))
            } else {
                ExecResult::Continue
            }
        }
        // Fallback: dispatch unrecognized commands to the Host system's Bash shell.
        _ => {
            let cargo_bin = format!("{}/.cargo/bin", home);
            let path_env = format!("{}:{}", cargo_bin, env::var("PATH").unwrap_or_default());

            let status = Command::new("bash")
                .env("PATH", path_env)
                .env("HOME", home)
                .env("USER", user)
                .envs([
                    ("RUSTUP_HOME", format!("{}/.rustup", home)),
                    ("CARGO_HOME", format!("{}/.cargo", home)),
                    ("RUSTUP_TOOLCHAIN", "stable".into()),
                ])
                .args(["-c", input])
                .status()
                .await;

            match status {
                Ok(_) => ExecResult::Continue,
                Err(e) => ExecResult::Error(format!("Bash Execution Error: {}", e)),
            }
        }
    }
}