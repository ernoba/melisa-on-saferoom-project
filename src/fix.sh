// ==============================================================================
// MELISA SERVER — REVISED: src/core/project_management.rs
//
// Perubahan dalam file ini:
//
//   FUNGSI DIREVISI:
//     1. update_project()  → _force diimplementasikan (dua mode: safe vs hard)
//                         → kode Laravel storage/ dihapus (hardcoded & project-specific)
//                         → numbering komentar diperbaiki
//     2. pull()            → validasi path & git repo ditambahkan
//                         → commit --allow-empty agar tidak gagal saat tidak ada perubahan
//                         → return bool untuk melaporkan sukses/gagal ke caller
//
//   FUNGSI DITAMBAHKAN:
//     3. check_user_in_project() → helper baru: cek apakah user punya workspace
//
//   TIDAK ADA FUNGSI YANG DIHAPUS.
//   new_project, invite, out_user, delete_project, list_projects, update_all_users
//   tidak diubah.
//
//   PATCH TERPISAH UNTUK executor.rs:
//     Bug #3: Argument order --pull terbalik → variable names diswap
//     (lihat bagian bawah file ini)
// ==============================================================================

use tokio::process::Command;
use std::path::Path;
use tokio::fs;

use crate::core::root_check::admin_check;
use crate::cli::color_text::{RED, GREEN, BLUE, YELLOW, BOLD, RESET};

pub const PROJECTS_MASTER: &str = "/opt/melisa/projects";


// =============================================================================
// HELPER BARU: check_user_in_project()
//
// Mengapa ditambahkan:
//   pull() dan update_project() keduanya perlu mengecek apakah user workspace
//   ada DAN merupakan git repo yang valid sebelum menjalankan operasi git.
//   Daripada duplikasi validasi, kita buat satu helper yang bisa di-reuse.
//
// Return: true jika /home/<username>/<project_name>/.git ada
// =============================================================================
async fn check_user_in_project(username: &str, project_name: &str) -> bool {
    let git_path = Path::new("/home")
        .join(username)
        .join(project_name)
        .join(".git");
    git_path.exists()
}


// =============================================================================
// FUNGSI DIREVISI: update_project()
//
// Perubahan dari versi sebelumnya:
//
//   1. `_force: bool` → `force: bool`
//      Parameter `_force` (prefix underscore = Rust suppress unused warning)
//      sebelumnya DIABAIKAN TOTAL. Fungsi selalu hard reset.
//      Sekarang diimplementasikan dua mode:
//
//      force=false (safe mode, dipanggil tanpa --force):
//        → git fetch origin
//        → git merge --ff-only origin/<branch>
//        → Jika ada uncommitted changes, TIDAK dihapus (aman)
//        → Jika ff-only gagal (branch diverged), beri pesan informatif tanpa crash
//
//      force=true (hard mode, dipanggil dengan --force atau dari post-receive hook):
//        → git clean -fd   (hapus untracked files)
//        → git fetch origin
//        → git reset --hard origin/<branch>   (hancurkan local changes)
//
//   2. Kode Laravel storage/ DIHAPUS:
//      Blok ini:
//        let storage_path = base_path.join("storage");
//        Command::new("sudo").args(&["chmod", "-R", "775", ...])
//        Command::new("sudo").args(&["chown", "-R", "{}:www-data", ...])
//      adalah sisa proyek spesifik "kasirku" yang tidak berkaitan dengan MELISA.
//      Komentar di kode asli bahkan menyebut "SECURITY FIX: Remove 'kasirku'"
//      tapi kodenya tidak dihapus, hanya di-rename path-nya.
//      Ini tidak boleh ada di tooling umum. DIHAPUS.
//
//   3. Numbering langkah diperbaiki:
//      Kode asli punya dua "// 1." yang ambigu.
// =============================================================================
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


// =============================================================================
// FUNGSI DIREVISI: pull()
//
// Nama fungsi ini membingungkan (vs git pull). Secara semantik ini adalah
// "admin force-sync dari workspace user ke master bare repo".
// Lebih tepat disebut "pull from user's workspace into master".
//
// Perubahan dari versi sebelumnya:
//
//   1. Validasi path dan git repo DITAMBAHKAN
//      Kode asli langsung menjalankan git add . tanpa cek apakah folder ada.
//      Jika user belum pernah di-invite ke project, ini crash diam-diam.
//
//   2. `--allow-empty` DITAMBAHKAN pada commit
//      Kode asli: git commit -m "Auto-sync..."
//      Jika tidak ada perubahan, commit GAGAL dengan exit code 1,
//      dan seluruh fungsi berhenti di sini tanpa mencapai push.
//      Dengan --allow-empty, pipeline selalu berjalan sampai push.
//
//   3. Return type diubah dari () menjadi bool
//      Caller (executor.rs) bisa tahu apakah pull berhasil.
//      update_all_users() juga bisa skip user yang gagal.
//
//   4. Admin check DIPINDAHKAN ke executor.rs (sudah ada di sana)
//      Tidak perlu double-check di dalam fungsi ini.
// =============================================================================
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


// =============================================================================
// PATCH UNTUK executor.rs — Bug #3: Argument order --pull terbalik
//
// Kode ASLI (SALAH) di executor.rs lines ~5604-5607:
//
//   let project_name = parts[2];   // <-- INI SEBENARNYA from_user!
//   let from_user = parts[3];      // <-- INI SEBENARNYA project_name!
//   pull(from_user, project_name).await;
//
// Perintah: melisa --pull alice myapp
//   parts[0] = "melisa"
//   parts[1] = "--pull"
//   parts[2] = "alice"     ← di-assign ke project_name (SALAH)
//   parts[3] = "myapp"     ← di-assign ke from_user (SALAH)
//   Hasilnya: pull("myapp", "alice") → mencari /home/myapp/alice → CRASH
//
// Error message di kode asli sendiri sudah benar:
//   "Usage: melisa --pull <from_user> <project_name>"
// Jadi parts[2] harusnya from_user, bukan project_name.
//
// Kode BENAR untuk executor.rs:
//
//   "--pull" => {
//       if !admin_check().await {
//           println!("{}[ERROR]{} Only Administrators can pull user workspaces.", RED, RESET);
//           return ExecResult::Continue;
//       }
//       if parts.len() < 4 {
//           println!(
//               "{}[ERROR]{} Usage: melisa --pull <from_user> <project_name>{}",
//               RED, BOLD, RESET
//           );
//           return ExecResult::Continue;
//       }
//       let from_user = parts[2];      // BENAR: alice
//       let project_name = parts[3];   // BENAR: myapp
//
//       let success = pull(from_user, project_name).await;
//       if !success {
//           return ExecResult::Continue;
//       }
//   },
//
// PERHATIAN: pull() sekarang return bool, pastikan tidak .await tanpa menangkap hasilnya.
// =============================================================================


// =============================================================================
// FUNGSI-FUNGSI YANG TIDAK DIUBAH (Referensi)
//
// new_project()       — OK, tidak perlu perubahan
// invite()            — OK, tidak perlu perubahan
// out_user()          — OK (catatan: access control grup belum ada, tapi itu 
//                        feature request, bukan bug dalam scope fungsi ini)
// delete_project()    — OK, tidak perlu perubahan
// list_projects()     — OK, tidak perlu perubahan
// update_all_users()  — OK, sequential tapi tidak perlu async paralel sekarang
// =============================================================================


// =============================================================================
// RINGKASAN SEMUA PERUBAHAN SERVER-SIDE
//
// FILE: src/core/project_management.rs
//   DIREVISI: update_project() — implementasi force flag, hapus kode Laravel
//   DIREVISI: pull()           — tambah validasi, --allow-empty, return bool
//   DITAMBAH: check_user_in_project() — helper reusable
//
// FILE: src/cli/executor.rs
//   DIREVISI: "--pull" arm — swap variable names from_user/project_name
//             dan tangani return value bool dari pull()
//
// TIDAK ADA FUNGSI YANG DIHAPUS.
// TIDAK ADA PERUBAHAN DI FUNGSI LAIN.
// =============================================================================