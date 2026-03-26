use tokio::process::Command; 
use std::path::Path; // Used for quick path existence checks
use tokio::fs; // Async file system operations

use crate::core::root_check::admin_check;
use crate::cli::color_text::{RED, GREEN, BLUE, YELLOW, BOLD, RESET};

pub const PROJECTS_MASTER: &str = "/opt/melisa/projects";

// new chhck user folder project
async fn check_user_in_project(username: &str, project_name: &str) -> bool {
    let git_path = Path::new("/home")
        .join(username)
        .join(project_name)
        .join(".git");
    git_path.exists()
}

/// Initializes a new master bare repository for a project.
/// This acts as the central source of truth for all users collaborating on the project.
pub async fn new_project(project_name: &str) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);
    
    // Create the master directory structure
    if let Err(e) = fs::create_dir_all(&master_path).await {
        eprintln!("{}[FATAL]{} Failed to create master directory structure: {}", RED, RESET, e);
        return;
    }

    // 1. Initialize Bare Repository with Shared Group mode
    // Using --shared=group ensures the directory automatically grants write access to the shared group
    let init_status = Command::new("git")
        .args(&["init", "--bare", "--shared=group", &master_path])
        .status().await;

    if let Ok(s) = init_status {
        if !s.success() {
            eprintln!("{}[ERROR]{} Git bare repository initialization failed.", RED, RESET);
            return;
        }
    }

    // --- UPGRADE: REGISTER SAFE DIRECTORY ---
    // Prevents Git's "dubious ownership" fatal error when multiple users interact with the same repo
    let _ = Command::new("git")
        .args(&["config", "--system", "--add", "safe.directory", &master_path])
        .status().await;

    // --- UPGRADE: PERMISSION & GROUP SECURITY ---
    // Ensure the folder is owned by the 'melisa' group (this group must exist on the host OS)
    // Permission 2775 (SetGID) ensures that any new files created inside will inherit the 'melisa' group
    let _ = Command::new("chown").args(&["-R", "root:melisa", &master_path]).status().await;
    let _ = Command::new("chmod").args(&["-R", "2775", &master_path]).status().await;
    
    // Additional Git configuration to explicitly allow group write permissions
    let _ = Command::new("git")
        .args(&["-C", &master_path, "config", "core.sharedRepository", "group"])
        .status().await;

    // 2. Setup Post-Receive Hook
    // This hook triggers automatically when a user pushes code, forcing all other users to update
    let hook_path = format!("{}/hooks/post-receive", master_path);
    // Execute via sudo so standard users pushing code can trigger a root-level system update
    let hook_content = format!("#!/bin/bash\nsudo melisa --update-all {}", project_name); 
    
    match fs::write(&hook_path, hook_content).await {
        Ok(_) => {
            let _ = Command::new("chmod").args(&["+x", &hook_path]).status().await;
            println!("{}[SUCCESS]{} Master Git repository '{}' initialized and security protocols applied.", GREEN, RESET, project_name);
        }
        Err(e) => eprintln!("{}[ERROR]{} Failed to write post-receive hook: {}", RED, RESET, e),
    }
}

/// Invites specific users to a project by cloning the master repository into their home directories.
pub async fn invite(project_name: &str, invited_users: &[&str]) {
    let master_path = format!("{}/{}", PROJECTS_MASTER, project_name);

    for username in invited_users {
        let user_project_path = format!("/home/{}/{}", username, project_name);
        
        // Clear any existing corrupted or old project folders for this user
        let _ = Command::new("rm").args(&["-rf", &user_project_path]).status().await;

        // UPGRADE: Ensure Git considers the master_path safe for this specific user before cloning
        let _ = Command::new("sudo")
            .args(&["-u", username, "git", "config", "--global", "--add", "safe.directory", &master_path])
            .status().await;

        // Attempt to clone the master repository
        let clone_status = Command::new("sudo")
            .args(&["-u", username, "git", "clone", &master_path, &user_project_path])
            .status().await;

        match clone_status {
            Ok(s) if s.success() => {
                // Guarantee the user owns the newly cloned directory
                let _ = Command::new("chown").args(&["-R", &format!("{}:{}", username, username), &user_project_path]).status().await;
                println!("{}[INVITED]{} User workspace for '{}' successfully provisioned.", GREEN, RESET, username);
            }
            _ => {
                // Fallback: If the master repo is completely empty, 'clone' will fail. 
                // We manually initialize the folder and link the remote instead.
                let _ = Command::new("sudo").args(&["-u", username, "mkdir", "-p", &user_project_path]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "init"]).status().await;
                let _ = Command::new("sudo").args(&["-u", username, "git", "-C", &user_project_path, "remote", "add", "origin", &master_path]).status().await;
                
                // Ensure ownership is correct even on manual initialization
                let _ = Command::new("chown").args(&["-R", &format!("{}:{}", username, username), &user_project_path]).status().await;
                
                println!("{}[WARNING]{} Master repository is empty. Workspace for '{}' initialized manually.", YELLOW, RESET, username);
            }
        }
    }
}

/// Automatically commits and pushes a user's local changes to the master repository.
pub async fn pull(username: &str, project_name: &str) -> bool {
    // 1. Validasi: workspace user harus ada dan merupakan git repo
    if !check_user_in_project(username, project_name).await {
        eprintln!(
            "{}[ERROR]{} User '{}' does not have a workspace for project '{}'.",
            RED, RESET, username, project_name
        );
        eprintln!(
            "{}[TIP]{} Run: melisa --invite {} {}",
            YELLOW, RESET, project_name, username
        );
        return false;
    }

    let user_path = format!("/home/{}/{}", username, project_name);

    // 2. Deteksi branch aktif
    let branch_out = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "branch", "--show-current"])
        .output().await;

    let branch = String::from_utf8_lossy(
        &branch_out.as_ref().map(|o| o.stdout.clone()).unwrap_or_default()
    ).trim().to_string();
    let branch = if branch.is_empty() { "master".to_string() } else { branch };

    println!(
        "{}[INFO]{} Pulling from '{}' workspace into master (Branch: {})...",
        BLUE, RESET, username, branch
    );

    // 3. Stage semua perubahan
    let _ = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "add", "."])
        .status().await;

    // 4. Commit — --allow-empty agar tidak gagal jika tidak ada perubahan baru
    //    (pipeline tetap jalan sampai push)
    let _ = Command::new("sudo")
        .args(&[
            "-u", username, "git", "-C", &user_path,
            "commit", "-m", "Admin force-pull: executed by MELISA",
            "--allow-empty"
        ])
        .status().await;

    // 5. Push ke master bare repo
    let push_status = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "push", "origin", &branch])
        .status().await;

    match push_status {
        Ok(s) if s.success() => {
            println!(
                "{}[SUCCESS]{} Workspace '{}@{}' successfully pulled into master.",
                GREEN, RESET, username, project_name
            );
            true
        },
        _ => {
            eprintln!(
                "{}[ERROR]{} Failed to push '{}' workspace to master. Possible divergence.",
                RED, RESET, username
            );
            eprintln!(
                "{}[TIP]{} Consider: melisa --update {} --force (to reset their workspace first)",
                YELLOW, RESET, project_name
            );
            false
        }
    }
}

/// Displays an overview of all projects. 
/// Admins see the root master projects; standard users see their local cloned workspaces.
pub async fn list_projects(home: &str) {
    let is_admin = admin_check().await; 
    println!("\n{}--- MELISA PROJECT DASHBOARD ---{}", BOLD, RESET);

    if is_admin {
        let output = Command::new("ls")
            .args(&["-1", PROJECTS_MASTER])
            .output().await;

        match output {
            Ok(out) if out.status.success() => {
                let list = String::from_utf8_lossy(&out.stdout);
                if list.trim().is_empty() {
                    println!("  {}No Master Projects have been established yet.{}", YELLOW, RESET);
                } else {
                    println!("{}Master Repositories (Root Infrastructure):{}", BOLD, RESET);
                    for project in list.lines() {
                        println!("  {} [MASTER] {}{}", GREEN, project, RESET);
                    }
                }
            },
            _ => eprintln!("{}[ERROR]{} Denied or failed access to the master projects directory.", RED, RESET),
        }
    } else {
        // Standard users only see directories in their home folder (excluding 'data/')
        let output = Command::new("ls")
            .args(&["-F", home]) 
            .output().await;

        if let Ok(out) = output {
            let list = String::from_utf8_lossy(&out.stdout);
            let mut found = false;
            
            println!("{}Active Workspace Assignments:{}", BOLD, RESET);
            for entry in list.lines() {
                if entry.ends_with('/') && entry != "data/" {
                    println!("  {} [WORKSPACE] {}{}", BLUE, entry.trim_end_matches('/'), RESET);
                    found = true;
                }
            }
            
            if !found {
                println!("  {}You have not been assigned to any active projects.{}", YELLOW, RESET);
            }
        }
    }
}

/// Completely obliterates a project from the master directory and from all users' local workspaces.
pub async fn delete_project(master_path: String, project_name: &str) {
    println!("{}[WARNING]{} Initiating total wipe sequence for project '{}'...", YELLOW, RESET, project_name);
    
    // 1. Destroy the master repository
    let _ = Command::new("rm").args(&["-rf", &master_path]).status().await;

    // 2. Iterate through all MELISA users and destroy their local clones
    let passwd_out = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = passwd_out {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
                
                // Ensure we only attempt to delete if the folder actually exists
                if Path::new(&user_project_path).exists() {
                    let _ = Command::new("rm").args(&["-rf", &user_project_path]).status().await;
                    println!("  {}[DELETED]{} Workspace removed for user '{}'.", YELLOW, RESET, username);
                }
            }
        }
        println!("{}[SUCCESS]{} Project '{}' completely eradicated from the server infrastructure.", GREEN, RESET, project_name);
    } else {
        eprintln!("{}[ERROR]{} Failed to retrieve user list during deletion sequence.", RED, RESET);
    }
}

/// Revokes project access for specific users by deleting their local workspace clones.
pub async fn out_user(targets: &[&str], project_name: &str) {
    for username in targets {
        let user_project_path = format!("/home/{}/{}", username, project_name);
        let status = Command::new("rm").args(&["-rf", &user_project_path]).status().await;

        match status {
            Ok(s) if s.success() => {
                println!("{}[REVOKED]{} User '{}' has been successfully removed from project '{}'.", YELLOW, RESET, username, project_name);
            }
            _ => eprintln!("{}[ERROR]{} Failed to purge project workspace for user '{}'.", RED, RESET, username),
        }
    }
}

/// Forcefully syncs a user's local workspace with the latest state of the master repository.
/// Typically triggered by the post-receive hook.
pub async fn update_project(username: &str, project_name: &str, force: bool) {
    // Validasi input: cegah path traversal
    if username.contains('/') || username.contains("..") 
       || project_name.contains('/') || project_name.contains("..") 
    {
        eprintln!("{}[ERROR]{} Invalid characters detected in input. Sync aborted.", RED, RESET);
        return;
    }

    let base_path = Path::new("/home").join(username).join(project_name);
    let user_path = base_path.to_str().unwrap_or_default().to_string();
    let git_path = base_path.join(".git");

    if !git_path.exists() {
        eprintln!(
            "{}[ERROR]{} Target path '{}' is not a valid Git repository. Sync aborted.",
            RED, RESET, user_path
        );
        return;
    }

    // Deteksi branch aktif
    let branch_out = Command::new("sudo")
        .args(&["-u", username, "git", "-C", &user_path, "branch", "--show-current"])
        .output().await;

    let mut branch = String::from_utf8_lossy(
        &branch_out.as_ref().map(|o| o.stdout.clone()).unwrap_or_default()
    ).trim().to_string();
    if branch.is_empty() { branch = "master".to_string(); }

    // --- PERCABANGAN BERDASARKAN force FLAG ---
    if force {
        // -------------------------------------------------------------------------
        // FORCE MODE: Hard reset — hancurkan semua perubahan lokal
        // Dipanggil oleh: post-receive hook (via --update-all) atau --force eksplisit
        // -------------------------------------------------------------------------
        println!(
            "{}[SYNC/FORCE]{} Hard reset for '{}@{}' (Branch: {})...",
            YELLOW, RESET, project_name, username, branch
        );

        // 1. Perbaiki ownership dulu agar git bisa manipulasi files
        let _ = Command::new("sudo")
            .args(&["chown", "-R", &format!("{}:{}", username, username), &user_path])
            .status().await;

        // 2. Hapus untracked files dan direktori
        let _ = Command::new("sudo")
            .args(&["-u", username, "git", "-C", &user_path, "clean", "-fd"])
            .status().await;

        // 3. Fetch data terbaru dari master bare repo
        let _ = Command::new("sudo")
            .args(&["-u", username, "git", "-C", &user_path, "fetch", "origin"])
            .status().await;

        // 4. Hard reset: workspace sekarang identik dengan master
        let status = Command::new("sudo")
            .args(&[
                "-u", username, "git", "-C", &user_path,
                "reset", "--hard", &format!("origin/{}", branch)
            ])
            .status().await;

        match status {
            Ok(s) if s.success() => {
                println!(
                    "{}[SUCCESS]{} '{}' (user: {}) forcefully synchronized to master state.",
                    GREEN, RESET, project_name, username
                );
            },
            _ => eprintln!(
                "{}[ERROR]{} Force sync failed for user '{}' project '{}'.",
                RED, RESET, username, project_name
            ),
        }

    } else {
        // -------------------------------------------------------------------------
        // SAFE MODE: Merge --ff-only — pertahankan uncommitted changes
        // Dipanggil oleh: melisa --update myapp (tanpa --force)
        // Cocok untuk: user yang mau pull perubahan teman tanpa kehilangan kerjaan sendiri
        // -------------------------------------------------------------------------
        println!(
            "{}[SYNC/SAFE]{} Safe update for '{}@{}' (Branch: {})...",
            BLUE, RESET, project_name, username, branch
        );

        // 1. Fetch tanpa mengubah apapun di workspace
        let fetch_status = Command::new("sudo")
            .args(&["-u", username, "git", "-C", &user_path, "fetch", "origin"])
            .status().await;

        if fetch_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
            eprintln!(
                "{}[ERROR]{} Failed to fetch from master for user '{}'. Check network/repo.",
                RED, RESET, username
            );
            return;
        }

        // 2. Fast-forward merge: hanya berhasil jika tidak ada divergence
        //    Jika ada uncommitted changes, git merge akan gagal — ini AMAN,
        //    user tidak kehilangan kerjaan mereka.
        let merge_status = Command::new("sudo")
            .args(&[
                "-u", username, "git", "-C", &user_path,
                "merge", "--ff-only", &format!("origin/{}", branch)
            ])
            .status().await;

        match merge_status {
            Ok(s) if s.success() => {
                println!(
                    "{}[SUCCESS]{} '{}' (user: {}) safely updated to latest master.",
                    GREEN, RESET, project_name, username
                );
            },
            Ok(_) => {
                // ff-only gagal: branch lokal sudah diverged dari master
                // Ini BUKAN error fatal — user mungkin punya commit lokal yang belum di-push
                println!(
                    "{}[INFO]{} Cannot fast-forward '{}' for user '{}'.",
                    YELLOW, RESET, project_name, username
                );
                println!(
                    "{}[TIP]{} Local branch has diverged from master.",
                    YELLOW, RESET
                );
                println!(
                    "{}[TIP]{} Use 'melisa --update {} --force' to discard local changes,",
                    YELLOW, RESET, project_name
                );
                println!(
                    "{}[TIP]{} or resolve manually: ssh to server → cd ~/{} → git status",
                    YELLOW, RESET, project_name
                );
            },
            Err(e) => {
                eprintln!("{}[ERROR]{} Merge command failed: {}", RED, RESET, e);
            }
        }
    }

    // NOTE: Blok storage/ Laravel yang ada di kode asli DIHAPUS dari sini.
    // Alasan: itu adalah kode proyek spesifik ("kasirku") yang tidak relevan
    // dengan MELISA sebagai tooling umum. Komentar di kode asli sendiri menyebut
    // "SECURITY FIX: Remove 'kasirku'" tapi implementasinya tidak pernah dihapus.
    //
    // Jika project Anda memerlukan post-sync hook (chmod storage/, build artifacts, dll),
    // implementasikan sebagai file terpisah: /home/<user>/<project>/.melisa-post-sync.sh
    // dan panggil dari sini jika file tersebut ada. Ini membuat MELISA tetap generic.
}

/// Triggers a hard update across ALL users assigned to a specific project.
/// This is the master command executed by the Git post-receive hook.
pub async fn update_all_users(project_name: &str) {
    let output = Command::new("grep")
        .args(&["/usr/local/bin/melisa", "/etc/passwd"])
        .output().await;

    if let Ok(out) = output {
        let result = String::from_utf8_lossy(&out.stdout);
        for line in result.lines() {
            // Extract usernames that are utilizing the MELISA shell
            if let Some(username) = line.split(':').next() {
                let user_project_path = format!("/home/{}/{}", username, project_name);
                
                // If the user has a workspace for this project, trigger their individual update protocol
                if Path::new(&user_project_path).exists() {
                    update_project(username, project_name, true).await;
                }
            }
        }
    }
}