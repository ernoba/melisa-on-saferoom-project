use tokio::process::Command; // Gunakan tokio

use crate::core::root_check::admin_check;
use crate::cli::color_text::{RED, GREEN, BLUE, YELLOW, BOLD, RESET};

pub const PROJECTS_MASTER: &str = "/opt/melisa/projects";

pub async fn new_project(project_name: &str) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);
    
    let _ = tokio::fs::create_dir_all(&master_path).await;

    // 1. Inisialisasi Bare Repo dengan mode Shared Group
    // Kita gunakan --shared=group agar folder otomatis bisa ditulis oleh grup yang sama
    let _ = Command::new("git")
        .args(&["init", "--bare", "--shared=group", &master_path])
        .status().await;

    // --- UPGRADE: REGISTER SAFE DIRECTORY ---
    let _ = Command::new("git")
        .args(&["config", "--system", "--add", "safe.directory", &master_path])
        .status().await;

    // --- UPGRADE: PERMISSION & GROUP ---
    // Pastikan folder dimiliki grup 'melisa' (pastikan grup ini sudah ada di sistem)
    // Gunakan 2775 agar file baru di dalam repo mewarisi grup induk (SetGID)
    let _ = Command::new("chown").args(&["-R", "root:melisa", &master_path]).status().await;
    let _ = Command::new("chmod").args(&["-R", "2775", &master_path]).status().await;
    
    // Konfigurasi tambahan agar git mengizinkan penulisan grup
    let _ = Command::new("git")
        .args(&["-C", &master_path, "config", "core.sharedRepository", "group"])
        .status().await;

    // 2. Setup Hook
    let hook_path = format!("{}/hooks/post-receive", master_path);
    // Gunakan sudo agar hook yang dipicu user biasa bisa menjalankan update-all sebagai root
    let hook_content = format!("#!/bin/bash\nsudo melisa --update-all {}", project_name); 
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
                // Pastikan user adalah pemilik folder setelah clone
                let _ = Command::new("chown").args(&["-R", &format!("{}:{}", username, username), &user_project_path]).status().await;
                println!("{}[INVITED]{} User '{}' berhasil disiapkan.", GREEN, RESET, username);
            }
            _ => {
                let _ = Command::new("sudo").args(&["-u", username, "mkdir", "-p", &user_project_path]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "init"]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "remote", "add", "origin", &master_path]).status().await;
                
                println!("{}[WARNING]{} Repo master kosong, folder '{}' diinisialisasi manual.", YELLOW, RESET, username);
            }
        }
    }
}

pub async fn pull(username: &str, project_name: &str) {
    let user_path = format!("/home/{}/{}", username, project_name);

    // Jalankan deteksi branch sebagai user tersebut agar tidak ada masalah permission .git
    let branch_out = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "branch", "--show-current"])
        .output().await;
    
    let branch = String::from_utf8_lossy(&branch_out.as_ref().map(|o| o.stdout.clone()).unwrap_or_default())
        .trim().to_string();
    let branch = if branch.is_empty() { "master".to_string() } else { branch };

    // 1. Git add & commit
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
    let is_admin = admin_check().await; 
    println!("{}--- MELISA PROJECT DASHBOARD ---{}", BOLD, RESET);

    if is_admin {
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
        let output = Command::new("ls")
            .args(&["-F", home]) 
            .output().await;

        if let Ok(out) = output {
            let list = String::from_utf8_lossy(&out.stdout);
            let mut found = false;
            
            println!("{}Your Active Projects (Branches):{}", BOLD, RESET);
            for entry in list.lines() {
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
    let _ = Command::new("rm").args(&["-rf", &master_path]).status().await;

    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
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

    if !std::path::Path::new(&git_path).exists() {
        eprintln!("{}[ERROR]{} Path '{}' bukan repository Git!", RED, RESET, user_path);
        return;
    }

    let branch_out = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "branch", "--show-current"])
        .output().await;
    
    let mut branch = String::from_utf8_lossy(&branch_out.as_ref().map(|o| o.stdout.clone()).unwrap_or_default())
        .trim().to_string();
    if branch.is_empty() { branch = "master".to_string(); }

    println!("{}[INFO]{} Sinkronisasi fisik project '{}' (Branch: {})...", BLUE, RESET, project_name, branch);

    // Kembalikan ownership agar user bisa memanipulasi file saat git reset
    let _ = Command::new("sudo")
        .args(&["chown", "-R", &format!("{}:{}", username, username), &user_path])
        .status().await;

    // Bersihkan file yang tidak terlacak agar tidak konflik
    let _ = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "clean", "-fd"])
        .status().await;

    // Ambil data terbaru dari master
    let _ = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "fetch", "origin"])
        .status().await;

    // Paksa update ke kondisi master terbaru
    let status = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "reset", "--hard", &format!("origin/{}", branch)])
        .status().await;

    match status {
        Ok(s) if s.success() => {
            println!("{}[SUCCESS]{} Project '{}' berhasil diperbarui sepenuhnya.", GREEN, RESET, project_name);
            
            // Pengaturan khusus untuk folder storage (Laravel) jika ada
            let storage_path = format!("{}/kasirku/storage", user_path);
            if std::path::Path::new(&storage_path).exists() {
                let _ = Command::new("sudo").args(&["chmod", "-R", "775", &storage_path]).status().await;
                let _ = Command::new("sudo").args(&["chown", "-R", &format!("{}:www-data", username), &storage_path]).status().await;
            }
        },
        _ => eprintln!("{}[ERROR]{} Gagal sinkronisasi fisik di server.", RED, RESET),
    }
}

pub async fn update_all_users(project_name: &str) {
    let output = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = output {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
                
                if std::path::Path::new(&user_project_path).exists() {
                    update_project(username, project_name, true).await;
                }
            }
        }
    }
}