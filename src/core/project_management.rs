use tokio::process::Command; // Gunakan tokio

use crate::core::root_check::admin_check;
use crate::cli::color_text::{RED, GREEN, BLUE, YELLOW, BOLD, RESET};

pub const PROJECTS_MASTER: &str = "/opt/melisa/projects";

pub async fn new_project(project_name: &str) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);
    
    // Buat folder master dan jalankan git init --bare
    let status = Command::new("git")
        .args(&["init", "--bare", &master_path])
        .status().await;

    match status {
        Ok(s) if s.success() => {
            // Berikan izin grup agar user lain bisa push/pull (asumsi ada group 'melisa')
            let _ = Command::new("chmod").args(&["-R", "770", &master_path]).status().await;
            println!("{}[SUCCESS]{} Master Git Repository '{}' siap.", GREEN, RESET, project_name);
        }
        _ => eprintln!("{}[ERROR]{} Gagal inisialisasi Git bare repo.", RED, RESET),
    }
}

pub async fn invite(project_name: &str, invited_users: &[&str]) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

    for username in invited_users {
        let user_project_path = format!("/home/{}/{}", username, project_name);
        
        // Clone dari master repo ke folder user
        let clone_status = Command::new("git")
            .args(&["clone", &master_path, &user_project_path])
            .status().await;

        match clone_status {
            Ok(s) if s.success() => {
                // Pastikan kepemilikan file kembali ke user tersebut
                let _ = Command::new("chown")
                    .args(&["-R", &format!("{}:{}", username, username), &user_project_path])
                    .status().await;
                println!("{}[INVITED]{} User '{}' telah meng-clone project.", GREEN, RESET, username);
            }
            _ => eprintln!("{}[ERROR]{} Gagal cloning untuk user '{}'.", RED, RESET, username),
        }
    }
}

// melisa --pull my-web afira
pub async fn pull(username: &str, project_name: &str) {
    let user_path = format!("/home/{}/{}", username, project_name);

    // 1. Jalankan git add . di folder user
    let _ = Command::new("git").current_dir(&user_path).args(&["add", "."]).status().await;

    // 2. Jalankan git commit
    let _ = Command::new("git")
        .current_dir(&user_path)
        .args(&["commit", "-m", "Auto-sync by MELISA"])
        .status().await;

    // 3. Jalankan git push ke master
    let push_status = Command::new("git")
        .current_dir(&user_path)
        .args(&["push", "origin", "main"]) // atau master
        .status().await;

    if let Ok(s) = push_status {
        if s.success() {
            println!("{}[SYNC]{} Perubahan dari '{}' berhasil di-push ke Master.", GREEN, RESET, username);
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

pub async fn update_project(username: &str, project_name: &str, force: bool) {
    let user_path = format!("/home/{}/{}", username, project_name);

    if force {
        // MODE 1: Force Reset (Menyamakan total dengan Master)
        // Bermanfaat kalau user ngodingnya error dan mau balik ke versi Master yang stabil
        println!("{}[ACTION]{} Melakukan force reset ke Master...", YELLOW, RESET);
        
        let _ = Command::new("git").current_dir(&user_path).args(&["fetch", "origin"]).status().await;
        let status = Command::new("git")
            .current_dir(&user_path)
            .args(&["reset", "--hard", "origin/main"]) // Sesuaikan 'main' atau 'master'
            .status().await;

        match status {
            Ok(s) if s.success() => println!("{}[SUCCESS]{} Folder kamu sekarang identik dengan Master.", GREEN, RESET),
            _ => eprintln!("{}[ERROR]{} Gagal melakukan force reset.", RED, RESET),
        }
    } else {
        // MODE 2: Normal Pull (Hanya ambil update terbaru)
        let status = Command::new("git")
            .current_dir(&user_path)
            .args(&["pull", "origin", "main"])
            .status().await;

        match status {
            Ok(s) if s.success() => {
                println!("{}[UPDATED]{} Project '{}' berhasil diperbarui.", GREEN, RESET, project_name);
            }
            _ => {
                eprintln!("{}[CONFLICT]{} Ada perbedaan yang bentrok! Gunakan mode --force jika ingin menimpa.", RED, RESET);
            }
        }
    }
}