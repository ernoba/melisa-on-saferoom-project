// src/distros/distro.rs
use std::process::Command;
use crate::core::container::DistroMetadata; // Pakai struct dari container.rs

pub fn get_lxc_distro_list() -> Vec<DistroMetadata> {
    let output = Command::new("lxc-create")
        .args(&["-t", "download", "--", "--list"])
        .output()
        .expect("Gagal fetch distro");

    let content = String::from_utf8_lossy(&output.stdout);
    let mut distros = Vec::new();

    for line in content.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() >= 4 && !line.contains("Distribution") && !line.contains("---") {
            let name = p[0].to_string();
            let release = p[1].to_string();
            let arch = p[2].to_string();
            let variant = p[3].to_string();
            
            let slug = generate_slug(&name, &release, &arch);

            let pkg_manager = match name.as_str() {
                "debian" | "ubuntu" | "kali" => "apt",
                "fedora" | "centos" | "rocky" | "almalinux" => "dnf",
                "alpine" => "apk",
                _ => "apt",
            }.to_string();

            distros.push(DistroMetadata { slug, name, release, arch, variant, pkg_manager });
        }
    }
    distros
}

fn generate_slug(name: &str, release: &str, arch: &str) -> String {
    let s_arch = match arch { "amd64" => "x64", "arm64" => "a64", _ => arch };
    format!("{}-{}-{}", &name[..name.len().min(3)], release, s_arch).to_lowercase()
}