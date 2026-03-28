use colored::*;
use rand::Rng;
use rand::seq::SliceRandom;
use sysinfo::System;
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;
use chrono::Local;

// --- KONFIGURASI ENGINE ---
const GLITCH_CHARS: &[u8] = b"01X#?!<>[]{}|";

// Pesan log sistem profesional (Menggantikan pesan imut)
const SYSTEM_LOGS: &[&str] = &[
    "Initializing core subsystem...",
    "Mapping virtual memory pages...",
    "Verifying LXC bridge connectivity...",
    "Loading security namespaces...",
    "Syncing environment variables...",
    "Hardening container isolation...",
    "Establishing encrypted session...",
    "Validating Saferoom architecture...",
];

pub fn display_melisa_banner() {
    clear_screen();
    
    // FASE 1: Boot Sequence (Animasi tetap ada, pesan diubah)
    system_boot_sequence();
    
    // FASE 2: Animasi Dekripsi Payload (Warna diubah ke Cyan)
    decrypt_core_animation();
    
    // FASE 3: Reconnaissance
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // FASE 4: Render Dashboard (Gaya Industrial)
    display_system_dashboard(&mut sys);
    
    // FASE 5: Security Enforcement
    enforce_isolation_directives();
}

fn clear_screen() {
    print!("{}[2J{}[1;1H", 27 as char, 27 as char);
    io::stdout().flush().unwrap();
}

fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

// --- FASE 1: SYSTEM BOOT SEQUENCE ---
fn system_boot_sequence() {
    let mut rng = rand::thread_rng();
    println!("\n  {}", ">> INITIALIZING CORE ENGINE...".cyan().bold());
    sleep_ms(400);
    
    for _ in 0..5 {
        let msg = SYSTEM_LOGS.choose(&mut rng).unwrap();
        let addr = format!("0x{:08X}", rng.gen_range(0x10000000_u32..0xFFFFFFFF_u32));
        println!("  {} [ OK ] {}", addr.bright_black(), msg.white());
        sleep_ms(rng.gen_range(150..400));
    }
}

// --- FASE 2: DECRYPT ANIMATION ---
fn decrypt_core_animation() {
    let mut rng = rand::thread_rng();
    let target_text = "M.E.L.I.S.A // SYSTEM_STABLE_ENVIRONMENT";
    let mut current: Vec<char> = (0..target_text.len()).map(|_| 'X').collect();
    
    print!("\n  {} ", "[ PROC ] DECRYPTING KERNEL:".cyan().bold());
    
    for i in 0..target_text.len() {
        for _ in 0..2 {
            current[i] = *GLITCH_CHARS.choose(&mut rng).unwrap() as char;
            let display: String = current.iter().collect();
            // Animasi glitch tetap ada, warna ganti ke Cyan/Blue
            print!("\r  {} {} ", "[ PROC ] DECRYPTING KERNEL:".cyan().bold(), display.on_cyan().black());
            io::stdout().flush().unwrap();
            sleep_ms(15);
        }
        current[i] = target_text.chars().nth(i).unwrap();
    }
    println!("\r  {} {} \n", "[ DONE ] KERNEL INITIALIZED:".bright_green().bold(), target_text.cyan().bold());
    sleep_ms(400);
}

// --- FASE 4: INDUSTRIAL DASHBOARD ---
fn display_system_dashboard(sys: &mut System) {
    clear_screen();

    let os_full_name = System::name().unwrap_or_else(|| "Linux".to_string());
    let host_name = System::host_name().unwrap_or_else(|| "saferoom".to_string());
    let cpu_info = sys.cpus().first().map(|cpu| cpu.brand().trim()).unwrap_or("Unknown CPU");

    // LOGO MELISA (Tetap besar tapi warna profesional)
    let melisa_text = vec![
        r#" ███╗   ███╗███████╗██║     ██║███████╗███████╗ "#,
        r#" ████╗ ████║██╔════╝██║     ██║██╔════╝██╔══██╗ "#,
        r#" ██╔████╔██║█████╗  ██║     ██║███████╗███████║ "#,
        r#" ██║╚██╔╝██║██╔══╝  ██║     ██║╚════██║██╔══██║ "#,
        r#" ██║ ╚═╝ ██║███████╗███████╗██║███████║██║  ██║ "#,
        r#" ╚═╝     ╚═╝╚══════╝╚══════╝╚═╝╚══════╝╚═╝  ╚═╝ "#,
        r#"    [ MANAGEMENT ENVIRONMENT LINUX SANDBOX ]    "#,
        r#""#,
        r#"[v - 0.1.3 | delta version]"#,
    ];

    for line in melisa_text {
        println!("  {}", line.cyan().bold());
    }

    // TELEMETRY DENGAN BORDER BERSIH
    println!("\n  {}", "┌─── SYSTEM TELEMETRY & STATUS ──────────────────────────────────────┐".bright_black());
    
    let time_now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let used_ram = sys.used_memory() / 1024 / 1024;
    let total_ram = sys.total_memory() / 1024 / 1024;
    let ram_percent = if total_ram > 0 { (used_ram as f64 / total_ram as f64 * 100.0) as u64 } else { 0 };

    let specs: Vec<(&str, String, Color)> = vec![
        ("TIMESTAMP ", time_now, Color::White),
        ("KERNEL_ID ", os_full_name.to_uppercase(), Color::Cyan),
        ("HOST_NODE ", host_name.to_uppercase(), Color::Cyan),
        ("PROCESSOR ", cpu_info.to_string(), Color::White),
        ("GPU_STATUS", get_gpu_info(), Color::White),
        ("RAM_USAGE ", format!("{}MB / {}MB ({}%)", used_ram, total_ram, ram_percent), if ram_percent > 80 { Color::Red } else { Color::Green }),
        ("----------", "".to_string(), Color::BrightBlack),
        ("PROTOCOL  ", "SECURE ISOLATION ACTIVE".to_string(), Color::Cyan),
        ("DIRECTIVE ", "MAXIMUM PERFORMANCE // ZERO INEFFICIENCY".to_string(), Color::Cyan),
    ];

    for (k, v, col) in specs {
        if k == "----------" {
            println!("  {} {}", "│".bright_black(), "------------------------------------------------------------------".bright_black());
            continue;
        }
        
        print!("  {} {} {} ", "│".bright_black(), k.bright_black().bold(), "::".cyan());
        io::stdout().flush().unwrap();
        
        // Animasi typing teks tetap dipertahankan
        for c in v.chars() {
            print!("{}", c.to_string().color(col));
            io::stdout().flush().unwrap();
            sleep_ms(5);
        }
        println!();
    }
    println!("  {}", "└────────────────────────────────────────────────────────────────────┘".bright_black());
}

// --- FASE 5: SECURITY ENFORCEMENT ---
fn enforce_isolation_directives() {
    println!("\n  {}", ">>> ALL SYSTEMS OPERATIONAL. SECURE SESSION GRANTED.".green().bold());
    print!("  {} ", "ENTER COMMAND:".bright_black().bold());
    io::stdout().flush().unwrap();
}

fn get_gpu_info() -> String {
    let output = Command::new("sh").arg("-c")
        .arg("lspci | grep -i vga | cut -d ':' -f3 | sed 's/\\[.*\\]//g' | head -n 1")
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() { "GENERIC_VGA".to_string() } else { s }
        },
        Err(_) => "OFFLINE".to_string(),
    }
}