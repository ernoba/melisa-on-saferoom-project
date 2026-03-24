use tokio::process::Command;
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::{self, OpenOptions};
use std::path::Path;
use std::process::Stdio;

use crate::core::root_check::{check_root, is_ssh_session};
use crate::cli::color_text::{GREEN, RED, CYAN, BOLD, RESET};
use crate::core::project_management::PROJECTS_MASTER;

pub async fn install() {
    // 1. Verifikasi Hak Akses & Keamanan Sesi
    if !check_root() {
        eprintln!("{}[ERROR] Setup harus dijalankan dengan hak akses root (Gunakan sudo).{}", RED, RESET);
        std::process::exit(1);
    }

    if is_ssh_session().await {
        println!("\n{}[SECURITY ALERT]{} Perintah 'setup' DILARANG via SSH!", RED, RESET);
        println!("{}Hanya user fisik (Host) yang boleh melakukan inisialisasi sistem.{}", BOLD, RESET);
        std::process::exit(1);
    }

    println!("\n{}MELISA SYSTEM & LXC INITIALIZATION (HOST MODE){}\n", BOLD, RESET);

    // 2. Verifikasi Lingkungan Lokal
    verify_data_environment().await;

    // 3. Instalasi Paket (Daftar perintah sistem)
    let commands = vec![
        ("Synchronizing package repositories", "dnf", vec!["update", "-y"]),
        ("Installing Virtualization & Bridge tools", "dnf", vec!["install", "-y", "lxc", "lxc-templates", "libvirt", "bridge-utils"]),
        ("Installing SSH & Security components", "dnf", vec!["install", "-y", "openssh-server", "firewalld"]),
        ("Loading veth kernel module", "modprobe", vec!["veth"]),
        ("Enabling LXC, SSH, & Firewall services", "systemctl", vec!["enable", "--now", "lxc.service", "lxc-net.service", "sshd", "firewalld"]),
    ];

    for (desc, prog, args) in commands {
        if !execute_step(desc, prog, &args).await {
            eprintln!("\n{}CRITICAL_FAILURE: Setup terminated at step '{}'{}", RED, desc, RESET);
            std::process::exit(1);
        }
    }

    // 4. Deployment & Konfigurasi Infrastruktur
    deploy_melisa_binary().await;
    setup_ssh_firewall().await;
    setup_lxc_network_quota().await;
    setup_projects_directory().await;
    configure_git_security().await;
    fix_shared_folder_permission("data").await;
    fix_uidmap_permissions().await;
    fix_system_privacy().await;

    // Mapping SubUID/SubGID untuk user yang menjalankan sudo
    if let Ok(user) = std::env::var("SUDO_USER") {
        setup_user_mapping(&user).await;
    }

    // 5. Finalisasi Shell
    register_melisa_shell().await;
    configure_sudoers_access().await;

    println!("\n{}VERIFYING SYSTEM CONFIGURATION...{}", BOLD, RESET);
    let _ = Command::new("lxc-checkconfig").status().await;

    println!("\n{}MELISA HOST DEPLOYMENT COMPLETED SUCCESSFULLY{}\n", GREEN, RESET);
    println!("{}STATUS: SSH Aktif, Jail Shell Terpasang, & Network Bridge Siap.{}", CYAN, RESET);
}

async fn execute_step(description: &str, program: &str, args: &[&str]) -> bool {
    println!("{} {}...", BOLD, description);
    let _ = io::stdout().flush().await;

    let status = Command::new(program)
        .args(args)
        .stdout(Stdio::inherit()) 
        .stderr(Stdio::inherit())
        .status()
        .await;

    match status {
        Ok(s) if s.success() => { 
            println!("{}[ OK ]{} {}", GREEN, RESET, description); 
            true 
        }
        _ => { 
            println!("{}[ FAILED ]{} {}", RED, RESET, description); 
            false 
        }
    }
}

async fn verify_data_environment() {
    let data_path = Path::new("data");
    println!("Verifying Local Data Environment...");

    if data_path.exists() {
        if let Ok(abs_path) = fs::canonicalize(data_path).await {
            println!("  {:<50} [ {}FOUND{} ]", "Data directory already exists", CYAN, RESET);
            println!("  {}Location: {}{}", BOLD, abs_path.display(), RESET);
            
            println!("  {}Contents:{}", BOLD, RESET);
            if let Ok(mut entries) = fs::read_dir(data_path).await {
                let mut has_files = false;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    println!("    - {}", entry.file_name().to_string_lossy());
                    has_files = true;
                }
                if !has_files { println!("    (Directory is empty)"); }
            }
        }
    } else {
        match fs::create_dir_all(data_path).await {
            Ok(_) => println!("  {:<50} [ {}CREATED{} ]", "New data directory created", GREEN, RESET),
            Err(_) => println!("  {:<50} [ {}FAILED{} ]", "Failed to create data directory", RED, RESET),
        }
    }
}

async fn deploy_melisa_binary() {
    let target_path = "/usr/local/bin/melisa";
    println!("\n{}REFRESHING BINARY: Overwriting /usr/local/bin/melisa...{}", BOLD, RESET);

    let current_exe = std::env::current_exe().expect("Gagal mendapatkan path biner");

    if fs::metadata(target_path).await.is_ok() {
        let _ = fs::remove_file(target_path).await;
        println!("  {:<50} [ {}CLEANED{} ]", "Old binary unlinked", CYAN, RESET);
    }

    let status = Command::new("cp")
        .args(&[current_exe.to_str().unwrap(), target_path])
        .status()
        .await;

    if let Ok(s) = status {
        if s.success() {
            // Set root ownership dan aktifkan SUID bit (4755)
            let _ = Command::new("chown").args(&["root:root", target_path]).status().await;
            let _ = Command::new("chmod").args(&["4755", target_path]).status().await;
            println!("  {:<50} [ {}UPDATED{} ]", "New version deployed (SUID set)", GREEN, RESET);
        }
    }
}

async fn setup_ssh_firewall() {
    println!("\nConfiguring Firewall for SSH Access...");
    let _ = Command::new("firewall-cmd").args(&["--add-service=ssh", "--permanent"]).stdout(Stdio::null()).status().await;
    let _ = Command::new("firewall-cmd").args(&["--reload"]).stdout(Stdio::null()).status().await;
    println!("  {:<50} [ {}OK{} ]", "Firewall SSH port 22 opened", GREEN, RESET);
}

async fn setup_lxc_network_quota() {
    let config_path = "/etc/lxc/lxc-usernet";
    if let Ok(user) = std::env::var("SUDO_USER") {
        let quota_rule = format!("{} veth lxcbr0 10\n", user);
        
        let content = fs::read_to_string(config_path).await.unwrap_or_default();
        if !content.contains(&quota_rule) {
            if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(config_path).await {
                let _ = file.write_all(quota_rule.as_bytes()).await;
                println!("  {:<50} [ {}OK{} ]", format!("Network quota for {} assigned", user), GREEN, RESET);
            }
        }
    }
}

async fn register_melisa_shell() {
    let shell_path = "/usr/local/bin/melisa";
    let cmd = format!("grep -qxF '{}' /etc/shells || echo '{}' >> /etc/shells", shell_path, shell_path);
    let _ = Command::new("sh").args(&["-c", &cmd]).status().await;
    println!("  {:<50} [ {}OK{} ]", "Shell registered in /etc/shells", GREEN, RESET);
}

async fn configure_sudoers_access() {
    let sudo_rule = "ALL ALL=(ALL) NOPASSWD: /usr/local/bin/melisa\n";
    let sudoers_file = "/etc/sudoers.d/melisa";

    if let Ok(mut file) = OpenOptions::new().create(true).write(true).truncate(true).open(sudoers_file).await {
        if file.write_all(sudo_rule.as_bytes()).await.is_ok() {
            let _ = Command::new("chmod").args(&["0440", sudoers_file]).status().await;
            println!("  {:<50} [ {}OK{} ]", "Sudoers rule deployed", GREEN, RESET);
        }
    }
}

async fn fix_uidmap_permissions() {
    let paths = ["/usr/bin/newuidmap", "/usr/bin/newgidmap"];
    for path in &paths {
        if Path::new(path).exists() {
            let _ = Command::new("chmod").args(&["u+s", path]).status().await;
        }
    }
    let _ = Command::new("chmod").args(&["+x", "/var/lib/lxc"]).status().await;
    println!("  {:<50} [ {}OK{} ]", "System traversal permissions fixed", GREEN, RESET);
}

async fn fix_shared_folder_permission(host_path: &str) {
    // Gunakan UID/GID 100000 untuk mapping unprivileged LXC
    let _ = Command::new("chown").args(&["-R", "100000:100000", host_path]).status().await;
}

async fn setup_projects_directory() {
    println!("\n{}Configuring Master Projects Infrastructure...{}", BOLD, RESET);

    let mkdir_status = Command::new("mkdir")
        .args(&["-p", PROJECTS_MASTER])
        .status()
        .await;

    match mkdir_status {
        Ok(s) if s.success() => {
            // Gunakan 1777 (Sticky Bit). 
            // Semua user bisa buat folder, tapi tidak bisa hapus folder orang lain.
            let chmod_status = Command::new("chmod")
                .args(&["1777", PROJECTS_MASTER])
                .status()
                .await;

            if let Ok(cs) = chmod_status {
                if cs.success() {
                    println!("  {:<50} [ {}OK{} ]", "Master projects directory open & secured", GREEN, RESET);
                } else {
                    println!("  {:<50} [ {}FAILED{} ]", "Failed to set permissions", RED, RESET);
                }
            }
        }
        _ => println!("  {:<50} [ {}FAILED{} ]", "Could not create projects directory", RED, RESET),
    }
}

async fn configure_git_security() {
    println!("\nConfiguring Global Git Security...");
    
    // --system agar konfigurasi ini tidak hilang saat user berganti
    let status = Command::new("git")
        .args(&["config", "--system", "--add", "safe.directory", "*"])
        .status()
        .await;

    match status {
        Ok(s) if s.success() => {
            println!("  {:<50} [ {}OK{} ]", "Global Git safe.directory set to '*'", GREEN, RESET);
        }
        _ => {
            println!("  {:<50} [ {}FAILED{} ]", "Failed to configure Git safe directory", RED, RESET);
        }
    }
}

async fn fix_system_privacy() {
    println!("\nHardening System Privacy...");
    // 711 pada /home mencegah user biasa melakukan 'ls /home' untuk melihat daftar user lain
    let _ = Command::new("chmod").args(&["711", "/home"]).status().await;
    println!("  {:<50} [ {}OK{} ]", "Directory /home is now unlistable", GREEN, RESET);
}

async fn setup_user_mapping(username: &str) {
    // Memberikan rentang UID/GID untuk subordinasi LXC
    let _ = Command::new("usermod")
        .args(&["--add-subuids", "100000-165535", "--add-subgids", "100000-165535", username])
        .status().await;
}