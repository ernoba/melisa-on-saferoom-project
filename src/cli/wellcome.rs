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

// Koleksi pesan lucu Melisa
const CUTE_MESSAGES: &[&str] = &[
    "Melisa lagi dandan bentar... (✿◠‿◠)",
    "Menyiapkan teh hangat untuk Tuan... ☕",
    "Sistem Operasi Imut Mode: ON 🌸",
    "Jangan lupa senyum hari ini ya! ✨",
    "Menghapus bug dengan kekuatan cinta... 💖",
    "Menghitung jumlah bintang di langit... ⭐",
    "Mengamankan Saferoom sambil rebahan... 🛌",
    "Melisa kangen deh... eh, maksudnya sistem siap! (⁄ ⁄•⁄ω⁄•⁄ ⁄)⁄",
];

pub fn display_melisa_banner() {
    clear_screen();
    
    // FASE 1: Boot Sequence dengan Pesan Lucu
    cute_boot_sequence();
    
    // FASE 2: Animasi Dekripsi Payload (Tetap ada, tapi warna pink/ungu)
    decrypt_payload_animation();
    
    // FASE 3: Reconnaissance
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // FASE 4: Render Dashboard (Ideologis tapi Pink)
    display_ideological_dashboard(&mut sys);
    
    // FASE 5: Enforce Directives
    enforce_saferoom_directives(&sys);
}

fn clear_screen() {
    print!("{}[2J{}[1;1H", 27 as char, 27 as char);
    io::stdout().flush().unwrap();
}

fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

// --- FASE 1: CUTE BOOT SEQUENCE ---
fn cute_boot_sequence() {
    let mut rng = rand::thread_rng();
    println!("\n  {}", ">> WAKING UP MELISA CORE... (* ^ ω ^)".magenta().bold());
    sleep_ms(400);
    
    for _ in 0..4 {
        let msg = CUTE_MESSAGES.choose(&mut rng).unwrap();
        let addr = format!("0x{:08X}", rng.gen_range(0x10000000_u32..0xFFFFFFFF_u32));
        println!("  {} :: {}", addr.bright_black(), msg.color(Color::BrightMagenta));
        sleep_ms(rng.gen_range(200..500));
    }
}

// --- FASE 2: DECRYPT ANIMATION ---
fn decrypt_payload_animation() {
    let mut rng = rand::thread_rng();
    let target_text = "M.E.L.I.S.A // SAFEROOM_UNLOCKED";
    let mut current: Vec<char> = (0..target_text.len()).map(|_| 'X').collect();
    
    print!("\n  {} ", "[♡] DECRYPTING KERNEL:".magenta().bold());
    
    for i in 0..target_text.len() {
        for _ in 0..3 {
            current[i] = *GLITCH_CHARS.choose(&mut rng).unwrap() as char;
            let display: String = current.iter().collect();
            print!("\r  {} {} ", "[♡] DECRYPTING KERNEL:".magenta().bold(), display.on_magenta().white());
            io::stdout().flush().unwrap();
            sleep_ms(15);
        }
        current[i] = target_text.chars().nth(i).unwrap();
    }
    println!("\r  {} {} \n", "[♡] DECRYPTING KERNEL:".bright_magenta().bold(), target_text.magenta().bold());
    sleep_ms(400);
}

// --- FASE 4: MINIMALIST & IDEOLOGICAL DASHBOARD ---
fn display_ideological_dashboard(sys: &mut System) {
    clear_screen();

    let os_full_name = System::name().unwrap_or_else(|| "Linux".to_string());
    let host_name = System::host_name().unwrap_or_else(|| "saferoom".to_string());
    let cpu_info = sys.cpus().first().map(|cpu| cpu.brand().trim()).unwrap_or("Unknown CPU");

    // LOGO OS 
    let (logo, logo_color) = match os_full_name.to_lowercase().as_str() {
        os if os.contains("fedora") => (vec![
            "        ______        ",
            "       /   ___|       ",
            "      /   /___        ",
            "     /   ____|        ",
            "    /   /             ",
            "   /___/              ",
            "                      ",
            "    FEDORA KERNEL     ",
        ], Color::BrightMagenta), // Fedora tapi warnanya pink/ungu
        _ => (vec![
            "       .---.          ",
            "      /     \\         ",
            "     (  @ @  )        ",
            "      )  V  (         ",
            "     /       \\        ",
            "    (         )       ",
            "     `-------'        ",
            "    SYSTEM KERNEL     ",
        ], Color::BrightMagenta),
    };

    let melisa_text = vec![
        r#" ███╗   ███╗███████╗██║     ██║███████╗███████╗ "#,
        r#" ████╗ ████║██╔════╝██║     ██║██╔════╝██╔══██╗ "#,
        r#" ██╔████╔██║█████╗  ██║     ██║███████╗███████║ "#,
        r#" ██║╚██╔╝██║██╔══╝  ██║     ██║╚════██║██╔══██║ "#,
        r#" ██║ ╚═╝ ██║███████╗███████╗██║███████║██║  ██║ "#,
        r#" ╚═╝     ╚═╝╚══════╝╚══════╝╚═╝╚══════╝╚═╝  ╚═╝ "#,
        r#"         [ SAFEROOM CORE ARCHITECTURE ]         "#,
    ];

    let max_lines = std::cmp::max(logo.len(), melisa_text.len());
    for i in 0..max_lines {
        let l_line = logo.get(i).unwrap_or(&"                      "); // 22 spasi kosong
        let m_line = melisa_text.get(i).unwrap_or(&"");
        
        println!("  {:<22}  {}", l_line.color(logo_color).bold(), m_line.magenta().bold());
    }

    // TELEMETRY DENGAN BORDER UNGU
    println!("\n  {}", "┏━━[ SYSTEM TELEMETRY & DIRECTIVES ]━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓".magenta());
    
    let time_now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let used_ram = sys.used_memory() / 1024 / 1024;
    let total_ram = sys.total_memory() / 1024 / 1024;
    let ram_percent = if total_ram > 0 { (used_ram as f64 / total_ram as f64 * 100.0) as u64 } else { 0 };

    let specs: Vec<(&str, String, Color)> = vec![
        ("TIMESTAMP ", time_now, Color::BrightBlack),
        ("TARGET_OS ", os_full_name.to_uppercase(), Color::White),
        ("KERNEL_ID ", host_name.to_uppercase(), Color::White),
        ("PROCESSOR ", cpu_info.to_string(), Color::BrightBlack),
        ("GRAPHICS  ", get_gpu_info(), Color::BrightBlack),
        ("RAM_USAGE ", format!("{}MB / {}MB ({}%)", used_ram, total_ram, ram_percent), if ram_percent > 80 { Color::Red } else { Color::White }),
        ("----------", "--------------------------------".to_string(), Color::Magenta),
        ("DOCTRINE  ", "\"AMOR FATI\" - FOCUS ON WHAT YOU CONTROL".to_string(), Color::BrightMagenta),
        ("OBJECTIVE ", "MAXIMUM R.O.I. // ZERO INEFFICIENCY".to_string(), Color::BrightMagenta),
    ];

    for (k, v, col) in specs {
        if k == "----------" {
            println!("  {} {}", "┃".magenta(), "------------------------------------------------------------------".magenta());
            continue;
        }
        
        print!("  {} {} {} ", "┃".magenta(), k.color(col).bold(), "::".magenta());
        io::stdout().flush().unwrap();
        
        for c in v.chars() {
            print!("{}", c.to_string().color(if col == Color::BrightMagenta { Color::BrightMagenta } else { Color::White }));
            io::stdout().flush().unwrap();
            sleep_ms(3);
        }
        println!();
    }
    println!("  {}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".magenta());
}

// --- FASE 5: SAFEROOM DIRECTIVE ENFORCEMENT ---
fn enforce_saferoom_directives(sys: &System) {
    println!("\n  {}", "[*] HUGGING PROCESSES TIGHTLY (ISOLATION)...".magenta().bold());
    sleep_ms(400);
    
    let mut process_count = 0;

    for (pid, process) in sys.processes() {
        if process_count > 3 { break; } 
        
        let proc_name = process.name().to_string_lossy();
        
        println!("      {} PID: {:<6} | TGT: {:<15} | {}", 
                 "♡".magenta(), 
                 pid, 
                 proc_name, 
                 "[ SECURED ]".bright_black());
        
        sleep_ms(100);
        process_count += 1;
    }
    
    println!("\n  {}", ">>> ALL SYSTEMS BOUND TO NON-LINEAR DIRECTIVES 🎀".bright_magenta().bold());
    println!("  {} \n", "AWAITING TENDER COMMAND...".blink().white());
}

fn get_gpu_info() -> String {
    let output = Command::new("sh").arg("-c")
        .arg("lspci | grep -i vga | cut -d ':' -f3 | sed 's/\\[.*\\]//g' | head -n 1")
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() { "ENCRYPTED_NODE".to_string() } else { s }
        },
        Err(_) => "OFFLINE_NODE".to_string(),
    }
}