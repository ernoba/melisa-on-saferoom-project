use std::process::{Command, Stdio};
use std::io::{self, Write};
use std::fs::{self, OpenOptions};
use std::path::Path;
use crate::cli::color_text::{GREEN, RED, CYAN, BOLD, RESET};

pub fn install() {
    println!("\n{}MELISA SYSTEM & LXC INITIALIZATION (HOST MODE){}\n", BOLD, RESET);

    // Langkah 1: Verifikasi dan Laporan Lingkungan Data
    verify_data_environment();

    // Langkah 2: Instalasi Paket Inti dan Layanan Sistem
    let commands = vec![
        ("Synchronizing package repositories", "dnf", vec!["update", "-y"]),
        ("Installing Virtualization & Bridge tools", "dnf", vec!["install", "-y", "lxc", "lxc-templates", "libvirt", "bridge-utils"]),
        ("Installing SSH & Security components", "dnf", vec!["install", "-y", "openssh-server", "firewalld"]),
        ("Loading veth kernel module", "modprobe", vec!["veth"]),
        ("Enabling LXC, SSH, & Firewall services", "systemctl", vec!["enable", "--now", "lxc.service", "lxc-net.service", "sshd", "firewalld"]),
    ];

    for (desc, prog, args) in commands {
        if !execute_step(desc, prog, &args) {
            eprintln!("\n{}CRITICAL_FAILURE: Setup terminated at step '{}'{}", RED, desc, RESET);
            std::process::exit(1);
        }
    }

    // Langkah 3: Deployment/Update Binary Melisa ke System Path
    deploy_melisa_binary();

    // Langkah 4: Konfigurasi Keamanan Jaringan (SSH & Quota)
    setup_ssh_firewall();
    setup_lxc_network_quota();

    // Langkah 5: Izin Eksekusi & Pemetaan ID Unprivileged
    fix_uidmap_permissions();

    if let Ok(user) = std::env::var("SUDO_USER") {
        setup_user_mapping(&user);
    }

    // Langkah 6: Registrasi Shell & Akses Sudoers Tanpa Password
    register_melisa_shell();
    configure_sudoers_access();

    println!("\n{}VERIFYING SYSTEM CONFIGURATION...{}", BOLD, RESET);
    let _ = Command::new("lxc-checkconfig").status();

    println!("\n{}MELISA HOST DEPLOYMENT COMPLETED SUCCESSFULLY{}\n", GREEN, RESET);
    println!("{}STATUS: SSH Aktif, Jail Shell Terpasang, & Network Bridge Siap.{}", CYAN, RESET);
    println!("{}R.O.I FOCUS: Jalankan 'useradd -s /usr/local/bin/melisa <user>' untuk menambah user.{}", BOLD, RESET);
}

fn verify_data_environment() {
    let data_path = Path::new("data");
    println!("Verifying Local Data Environment...");

    if data_path.exists() {
        if let Ok(abs_path) = fs::canonicalize(data_path) {
            println!("  {:<50} [ {}FOUND{} ]", "Data directory already exists", CYAN, RESET);
            println!("  {}Location: {}{}", BOLD, abs_path.display(), RESET);
            
            // Melaporkan isi file di dalam folder data
            println!("  {}Contents:{}", BOLD, RESET);
            if let Ok(entries) = fs::read_dir(data_path) {
                let mut has_files = false;
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    println!("    - {}", file_name.to_string_lossy());
                    has_files = true;
                }
                if !has_files {
                    println!("    (Directory is empty)");
                }
            }
        }
    } else {
        match fs::create_dir_all(data_path) {
            Ok(_) => println!("  {:<50} [ {}CREATED{} ]", "New data directory created", GREEN, RESET),
            Err(_) => println!("  {:<50} [ {}FAILED{} ]", "Failed to create data directory", RED, RESET),
        }
    }
    println!();
}

fn deploy_melisa_binary() {
    let target_path = "/usr/local/bin/melisa";
    println!("\n{}REFRESHING BINARY: Overwriting /usr/local/bin/melisa...{}", BOLD, RESET);

    let current_exe = std::env::current_exe().expect("Gagal mendapatkan path biner saat ini");

    // 1. Hapus biner lama secara paksa agar tidak ada konflik inode
    if Path::new(target_path).exists() {
        let _ = std::fs::remove_file(target_path); 
        println!("  {:<50} [ {}CLEANED{} ]", "Old binary unlinked", CYAN, RESET);
    }

    // 2. Salin biner baru
    let status = Command::new("cp")
        .args(&[current_exe.to_str().unwrap(), target_path])
        .status();

    if let Ok(s) = status {
        if s.success() {
            // 3. Set izin root dan SUID agar bisa dipanggil via sudo oleh siapa pun
            let _ = Command::new("chown").args(&["root:root", target_path]).status();
            let _ = Command::new("chmod").args(&["4755", target_path]).status();
            println!("  {:<50} [ {}UPDATED{} ]", "New version deployed", GREEN, RESET);
        }
    }
}

fn setup_ssh_firewall() {
    println!("\nConfiguring Firewall for SSH Access...");
    let _ = Command::new("firewall-cmd").args(&["--add-service=ssh", "--permanent"]).stdout(Stdio::null()).status();
    let _ = Command::new("firewall-cmd").args(&["--reload"]).stdout(Stdio::null()).status();
    println!("  {:<50} [ {}OK{} ]", "Firewall SSH port 22 opened", GREEN, RESET);
}

fn setup_lxc_network_quota() {
    println!("\nConfiguring LXC network quota for unprivileged containers...");
    let config_path = "/etc/lxc/lxc-usernet";
    
    if let Ok(user) = std::env::var("SUDO_USER") {
        let quota_rule = format!("{} veth lxcbr0 10\n", user);
        
        // Memeriksa apakah aturan sudah ada agar tidak duplikat
        let content = fs::read_to_string(config_path).unwrap_or_default();
        if !content.contains(&quota_rule) {
            if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(config_path) {
                let _ = file.write_all(quota_rule.as_bytes());
                println!("  {:<50} [ {}OK{} ]", format!("Network quota for {} assigned", user), GREEN, RESET);
            }
        } else {
            println!("  {:<50} [ {}SKIP{} ]", "Network quota already configured", CYAN, RESET);
        }
    }
}

fn register_melisa_shell() {
    let shell_path = "/usr/local/bin/melisa";
    println!("\nRegistering Melisa as a valid system shell...");
    
    let cmd = format!("grep -qxF '{}' /etc/shells || echo '{}' >> /etc/shells", shell_path, shell_path);
    let status = Command::new("sh").args(&["-c", &cmd]).status();
    
    match status {
        Ok(s) if s.success() => println!("  {:<50} [ {}OK{} ]", "Shell registered in /etc/shells", GREEN, RESET),
        _ => println!("  {:<50} [ {}FAILED{} ]", "Failed to register shell", RED, RESET),
    }
}

fn configure_sudoers_access() {
    println!("\nConfiguring Sudoers for zero-password jail entry...");
    let sudo_rule = "ALL ALL=(ALL) NOPASSWD: /usr/local/bin/melisa\n";
    let sudoers_file = "/etc/sudoers.d/melisa";

    if let Ok(mut file) = OpenOptions::new().create(true).write(true).truncate(true).open(sudoers_file) {
        if let Ok(_) = file.write_all(sudo_rule.as_bytes()) {
            let _ = Command::new("chmod").args(&["0440", sudoers_file]).status();
            println!("  {:<50} [ {}OK{} ]", "Sudoers rule deployed", GREEN, RESET);
        }
    }
}

fn execute_step(description: &str, program: &str, args: &[&str]) -> bool {
    print!("  {:<50}", description);
    io::stdout().flush().unwrap();
    let status = Command::new(program).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
    match status {
        Ok(s) if s.success() => { println!("[ {}OK{} ]", GREEN, RESET); true }
        _ => { println!("[ {}FAILED{} ]", RED, RESET); false }
    }
}

fn fix_uidmap_permissions() {
    println!("\nApplying binary permission overrides...");
    let paths = ["/usr/bin/newuidmap", "/usr/bin/newgidmap"];
    for path in &paths {
        if Path::new(path).exists() {
            let _ = Command::new("chmod").args(&["u+s", path]).status();
        }
    }
    // Tambahkan ini: Izin agar user bisa 'mengintip' folder container sebelum attach
    let _ = Command::new("chmod").args(&["+x", "/var/lib/lxc"]).status();
    let _ = Command::new("chmod").args(&["+x", "/var/lib"]).status();
    println!("  {:<50} [ {}OK{} ]", "System traversal permissions fixed", GREEN, RESET);
}

fn setup_user_mapping(username: &str) {
    let _ = Command::new("usermod")
        .args(&["--add-subuids", "100000-165535", "--add-subgids", "100000-165535", username])
        .status();
}