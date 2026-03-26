// src/distros/distro.rs
use tokio::process::Command;
use crate::core::container::DistroMetadata;
// Upgraded to tokio::fs for 100% non-blocking asynchronous file operations
use tokio::fs; 
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::time::sleep;

const GLOBAL_CACHE: &str = "/tmp/melisa_global_distros.cache";
const LOCK_FILE: &str = "/tmp/melisa_distro.lock";
const CACHE_EXPIRY: u64 = 3600; // Cache lifespan set to 1 hour (3600 seconds)

/// Retrieves the list of available LXC distributions.
/// Implements an advanced concurrent caching mechanism to bypass the slow lxc-download command.
/// Returns a tuple containing the list of metadata and a boolean indicating if the data was served from cache.
pub async fn get_lxc_distro_list() -> (Vec<DistroMetadata>, bool) {
    let cache_exists = Path::new(GLOBAL_CACHE).exists();
    
    // 1. FAST PATH
    if cache_exists && is_cache_fresh(GLOBAL_CACHE).await && !Path::new(LOCK_FILE).exists() {
        if let Ok(content) = fs::read_to_string(GLOBAL_CACHE).await {
            return (parse_distro_list(&content), true);
        }
    }

    // 2. CONCURRENCY CONTROL WITH ANTI-STALE LOCK
    let mut retry_count = 0;
    let max_retries = 40; 

    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true) 
            .open(LOCK_FILE)
            .await 
        {
            Ok(_) => break, // LOCK ACQUIRED
            Err(_) => {
                // CHECK STALE LOCK: Remove the lock if it is older than 60 seconds (stuck)
                if let Ok(meta) = fs::metadata(LOCK_FILE).await {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                            if let Ok(last_mod) = mtime.duration_since(UNIX_EPOCH) {
                                if now.as_secs() - last_mod.as_secs() > 60 {
                                    let _ = fs::remove_file(LOCK_FILE).await;
                                    continue; // Try to acquire the lock again in the next loop iteration
                                }
                            }
                        }
                    }
                }

                if retry_count >= max_retries {
                    if cache_exists {
                        if let Ok(old_content) = fs::read_to_string(GLOBAL_CACHE).await {
                            return (parse_distro_list(&old_content), true);
                        }
                    }
                    break; 
                }

                if !Path::new(LOCK_FILE).exists() {
                    if let Ok(content) = fs::read_to_string(GLOBAL_CACHE).await {
                        return (parse_distro_list(&content), true);
                    }
                }

                sleep(Duration::from_millis(500)).await;
                retry_count += 1;
            }
        }
    }

    // 3. EXECUTE DATA RETRIEVAL
    // [UPGRADE]: Added -H flag to force HOME=/root. This prevents GPG from crashing 
    // when trying to read/write to the standard user's .gnupg directory.
    let output = Command::new("sudo")
        .args(&["-n", "-H", "/usr/share/lxc/templates/lxc-download", "--list"])
        .output()
        .await;

    let result = match output {
        // [CRITICAL FIX]: Check if stdout contains "Distribution" instead of strictly relying on exit code 0
        Ok(out) if out.status.success() || (!out.stdout.is_empty() && (String::from_utf8_lossy(&out.stdout).contains("Distribution") || String::from_utf8_lossy(&out.stdout).contains("DIST"))) => {
            let content = String::from_utf8_lossy(&out.stdout);
            if !content.is_empty() {
                let _ = fs::write(GLOBAL_CACHE, content.to_string()).await;
                let _ = Command::new("sudo").args(&["chmod", "666", GLOBAL_CACHE]).status().await;
                (parse_distro_list(&content), false)
            } else {
                (Vec::new(), false)
            }
        },
        Ok(out) => {
            eprintln!("\n[DEBUG] Main script failed. Error: {}", String::from_utf8_lossy(&out.stderr));
            
            // PRE-EMPTIVE CLEANUP
            let _ = Command::new("sudo")
                .args(&["-n", "lxc-destroy", "-n", "MELISA_PROBE_UNUSED", "-f"])
                .output()
                .await;

            // FALLBACK PROTOCOL
            // [UPGRADE]: Added -H flag for GPG safety
            let fallback = Command::new("sudo")
                .args(&["-n", "-H", "lxc-create", "-n", "MELISA_PROBE_UNUSED", "-t", "download", "--", "--list"])
                .output()
                .await;
            
            let mut final_result = (Vec::new(), false);

            if let Ok(fb_out) = fallback {
                let content = String::from_utf8_lossy(&fb_out.stdout);
                
                // [CRITICAL FIX]: lxc-create will always return Exit Code 1 here because no container is generated.
                // We MUST intercept the stdout stream and look for the data directly!
                if !content.is_empty() && (content.contains("Distribution") || content.contains("DIST")) {
                    let _ = fs::write(GLOBAL_CACHE, content.to_string()).await;
                    let _ = Command::new("sudo").args(&["chmod", "666", GLOBAL_CACHE]).status().await;
                    final_result = (parse_distro_list(&content), false);
                } else {
                    eprintln!("[DEBUG] Fallback failed. Error: {}", String::from_utf8_lossy(&fb_out.stderr));
                }
            }

            // POST-EXECUTION CLEANUP
            let _ = Command::new("sudo")
                .args(&["-n", "lxc-destroy", "-n", "MELISA_PROBE_UNUSED", "-f"])
                .output()
                .await;

            final_result
        },
        Err(e) => {
            eprintln!("\n[DEBUG] Failed to execute Command. Error: {}", e);
            (Vec::new(), false)
        }
    };

    let _ = fs::remove_file(LOCK_FILE).await;
    
    result
}

/// Helper function to determine if the cache file is still within its valid lifespan.
/// Upgraded to use asynchronous metadata retrieval.
async fn is_cache_fresh(path: &str) -> bool {
    if let Ok(meta) = fs::metadata(path).await {
        if let Ok(mtime) = meta.modified() {
            if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                if let Ok(last_mod) = mtime.duration_since(UNIX_EPOCH) {
                    return now.as_secs() - last_mod.as_secs() < CACHE_EXPIRY;
                }
            }
        }
    }
    false
}

/// Parses the raw string output from LXC into structured DistroMetadata objects.
fn parse_distro_list(content: &str) -> Vec<DistroMetadata> {
    let mut distros = Vec::new();
    for line in content.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        
        // Filter out headers and separator lines from the lxc output
        if p.len() >= 4 && !line.contains("Distribution") && !line.contains("DIST") && !line.contains("---") {
            let name = p[0].to_string();
            let release = p[1].to_string();
            let arch = p[2].to_string();
            let variant = p[3].to_string();
            
            let slug = generate_slug(&name, &release, &arch);
            
            // Map the Linux distribution to its native package manager
            let pkg_manager = match name.as_str() {
                "debian" | "ubuntu" | "kali" => "apt",
                "fedora" | "centos" | "rocky" | "almalinux" => "dnf",
                "alpine" => "apk",
                "archlinux" => "pacman",
                "opensuse" => "zypper",
                _ => "apt", // Default fallback
            }.to_string();

            distros.push(DistroMetadata { slug, name, release, arch, variant, pkg_manager });
        }
    }
    distros
}

/// Generates a unique, short identifier (slug) for the container interface.
fn generate_slug(name: &str, release: &str, arch: &str) -> String {
    let s_arch = match arch { 
        "amd64" => "x64", 
        "arm64" => "a64", 
        "i386" => "x86",
        _ => arch 
    };
    // Truncate the distro name to 3 characters for cleaner CLI output
    format!("{}-{}-{}", &name[..name.len().min(3)], release, s_arch).to_lowercase()
}