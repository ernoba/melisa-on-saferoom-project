use tokio::process::Command; // Gunakan tokio

use crate::core::root_check::admin_check;
use crate::cli::color_text::{RED, GREEN, BLUE, YELLOW, BOLD, RESET};

pub const PROJECTS_MASTER: &str = "/opt/melisa/projects";

pub async fn new_project(project_name: &str) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);
    
    let _ = tokio::fs::create_dir_all(&master_path).await;

    // 1. Inisialisasi Bare Repo
    let _ = Command::new("git")
        .args(&["init", "--bare", "--shared", &master_path])
        .status().await;

    // --- UPGRADE: REGISTER SAFE DIRECTORY ---
    // Agar Git tidak menganggap folder ini "dubious" saat diakses user non-root
    let _ = Command::new("git")
        .args(&["config", "--system", "--add", "safe.directory", &master_path])
        .status().await;

    // --- UPGRADE: PERMISSION 777 ---
    // Kita berikan 777 pada Bare Repo agar 'git-receive-pack' bisa membuat folder temporary 
    // saat ada user yang melakukan push (Solusi Unpacker Error)
    let _ = Command::new("chmod").args(&["-R", "777", &master_path]).status().await;

    // 2. Setup Hook
    // 2. Setup Hook
    let hook_path = format!("{}/hooks/post-receive", master_path);
    let hook_content = format!("#!/bin/bash\nmelisa --update-all {}", project_name); 
    let _ = tokio::fs::write(&hook_path, hook_content).await;
    let _ = Command::new("chmod").args(&["+x", &hook_path]).status().await;

    println!("{}[SUCCESS]{} Master Git '{}' siap dan sudah di-patch aman.", GREEN, RESET, project_name);
}

pub async fn invite(project_name: &str, invited_users: &[&str]) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

    for username in invited_users {
        let user_project_path = format!("/home/{}/{}", username, project_name);
        let _ = Command::new("rm").args(&["-rf", &user_project_path]).status().await;

        // UPGRADE: Pastikan Git menganggap master_path aman bagi user ini sebelum clone
        let _ = Command::new("sudo")
            .args(&["-u", username, "git", "config", "--global", "--add", "safe.directory", &master_path])
            .status().await;

        let clone_status = Command::new("sudo")
            .args(&["-u", username, "git", "clone", &master_path, &user_project_path])
            .status().await;

        match clone_status {
            Ok(s) if s.success() => {
                println!("{}[INVITED]{} User '{}' berhasil disiapkan.", GREEN, RESET, username);
            }
            _ => {
                // Jika clone gagal (biasanya karena repo master masih kosong/empty)
                // Kita buatkan folder manual dan inisialisasi remote-nya
                let _ = Command::new("sudo").args(&["-u", username, "mkdir", "-p", &user_project_path]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "init"]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "remote", "add", "origin", &master_path]).status().await;
                
                println!("{}[WARNING]{} Repo master kosong, folder '{}' diinisialisasi manual.", YELLOW, RESET, username);
            }
        }
    }
}

// melisa --pull my-web afira
pub async fn pull(username: &str, project_name: &str) {
    let user_path = format!("/home/{}/{}", username, project_name);

    // UPGRADE: Deteksi branch aktif di folder user secara otomatis
    let branch_out = Command::new("git")
        .current_dir(&user_path)
        .args(&["branch", "--show-current"])
        .output().await;
    
    let branch = String::from_utf8_lossy(&branch_out.map(|o| o.stdout).unwrap_or_default())
        .trim().to_string();
    let branch = if branch.is_empty() { "master".to_string() } else { branch };

    // 1. Git add & commit (dilakukan sebagai user tersebut)
    let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_path, "add", "."]).status().await;
    let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_path, "commit", "-m", "Auto-sync by MELISA"]).status().await;

    // 2. Push ke master
    let push_status = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "push", "origin", &branch])
        .status().await;

    if let Ok(s) = push_status {
        if s.success() {
            println!("{}[SYNC]{} Perubahan dari '{}' (@{}) berhasil dikirim.", GREEN, RESET, project_name, branch);
        }
    }
}

pub async fn list_projects(home: &str) {
    let is_admin = admin_check().await; //
    println!("{}--- MELISA PROJECT DASHBOARD ---{}", BOLD, RESET);

    if is_admin {
        // Logika Admin: Melihat semua Master Project di Root
        let output = Command::new("ls")
            .args(&["-1", PROJECTS_MASTER])
            .output().await;

        match output {
            Ok(out) if out.status.success() => {
                let list = String::from_utf8_lossy(&out.stdout);
                if list.trim().is_empty() {
                    println!("  {}Belum ada Master Project yang dibuat.{}", YELLOW, RESET);
                } else {
                    println!("{}Master Projects (Root):{}", BOLD, RESET);
                    for project in list.lines() {
                        println!("  {} [MASTER] {}{}", GREEN, project, RESET);
                    }
                }
            },
            _ => eprintln!("{}[ERROR]{} Gagal mengakses direktori master.", RED, RESET),
        }
    } else {
        // Logika User Reguler: Melihat project di folder Home mereka
        // Kita tampilkan folder di HOME kecuali folder sistem 'data'
        let output = Command::new("ls")
            .args(&["-F", home]) 
            .output().await;

        if let Ok(out) = output {
            let list = String::from_utf8_lossy(&out.stdout);
            let mut found = false;
            
            println!("{}Your Active Projects (Branches):{}", BOLD, RESET);
            for entry in list.lines() {
                // Filter: Hanya tampilkan folder (akhiran /) dan bukan folder sistem 'data/'
                if entry.ends_with('/') && entry != "data/" {
                    println!("  {} [BRANCH] {}{}", BLUE, entry.trim_end_matches('/'), RESET);
                    found = true;
                }
            }
            
            if !found {
                println!("  {}Kamu belum diundang ke project manapun.{}", YELLOW, RESET);
            }
        }
    }
}

pub async fn delete_project(master_path: String, project_name: &str) {
    // 2. Hapus Master Project (Root)
    let _ = Command::new("rm").args(&["-rf", &master_path]).status().await;

    // 3. Hapus otomatis dari SEMUA user yang terdaftar di Melisa
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
                // Hapus folder project di home user jika ada
                let _ = Command::new("rm").args(&["-rf", &user_project_path]).status().await;
                println!("{} telah di hapus dari {}", project_name, username);
            }
        }
        println!("{} berhasil di hapus dari dir master", project_name);
    }
}

pub async fn out_user(targets: &[&str], project_name: &str) {
    for username in targets {
        let user_project_path = format!("/home/{}/{}", username, project_name);
        
        // Eksekusi penghapusan folder di home user tersebut
        let status = Command::new("rm").args(&["-rf", &user_project_path]).status().await;

        match status {
            Ok(s) if s.success() => {
                println!("{}[OUT]{} User '{}' telah dikeluarkan dari project '{}'.", YELLOW, RESET, username, project_name);
            }
            _ => eprintln!("{}[ERROR]{} Gagal menghapus folder project untuk user '{}'.", RED, RESET, username),
        }
    }
}

pub async fn update_project(username: &str, project_name: &str, _force: bool) {
    let user_path = format!("/home/{}/{}", username, project_name);
    let git_path = format!("{}/.git", user_path);

    // 1. Validasi keberadaan Repo
    if !std::path::Path::new(&git_path).exists() {
        eprintln!("{}[ERROR]{} Path '{}' bukan repository Git!", RED, RESET, user_path);
        return;
    }

    // 2. Deteksi branch secara otomatis
    let branch_out = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "branch", "--show-current"])
        .output().await;
    
    let mut branch = String::from_utf8_lossy(&branch_out.as_ref().map(|o| o.stdout.clone()).unwrap_or_default())
        .trim().to_string();
    if branch.is_empty() { branch = "master".to_string(); }

    println!("{}[INFO]{} Sinkronisasi fisik project '{}' (Branch: {})...", BLUE, RESET, project_name, branch);

    // --- FIX: KEMBALIKAN OWNERSHIP KE USER ---
    // Pastikan user adalah pemilik sah dari seluruh file di folder project-nya 
    // agar Git tidak kena 'Permission denied' saat reset/clean.
    let _ = Command::new("sudo")
        .args(&["chown", "-R", &format!("{}:{}", username, username), &user_path])
        .status().await;

    // 3. MEMBERSIHKAN HAMBATAN (PENTING!)
    let _ = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "clean", "-fd"])
        .status().await;

    // 4. AMBIL DATA TERBARU (FETCH)
    let _ = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "fetch", "origin"])
        .status().await;

    // 5. PAKSA UPDATE (RESET --HARD)
    let status = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "reset", "--hard", &format!("origin/{}", branch)])
        .status().await;

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} Project '{}' berhasil diperbarui sepenuhnya.", GREEN, RESET, project_name);
            
            // Opsional: Karena tadi kita chown semuanya ke user, kita perlu pastikan
            // folder storage Laravel tetap bisa diakses web server (jika diperlukan)
            let storage_path = format!("{}/kasirku/storage", user_path);
            if std::path::Path::new(&storage_path).exists() {
                let _ = Command::new("sudo").args(&["chmod", "-R", "775", &storage_path]).status().await;
            }
        },
        _ => eprintln!("{}[ERROR]{} Gagal sinkronisasi fisik di server.", RED, RESET),
    }
}

pub async fn update_all_users(project_name: &str) {
    // 1. Ambil semua user yang pakai shell melisa
    let output = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = output {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
                
                // --- FIX: Pengecekan Eksistensi Folder ---
                // Hanya jalankan update jika folder project ADA di home user tersebut
                if std::path::Path::new(&user_project_path).exists() {
                    // Jalankan update dengan force = true agar file untracked (.env) tertimpa
                    update_project(username, project_name, true).await;
                }
            }
        }
    }
}