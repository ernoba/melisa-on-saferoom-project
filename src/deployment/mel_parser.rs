use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MelManifest {
    pub project:    ProjectSection,
    pub container:  ContainerSection,
    #[serde(default)]
    pub env:        HashMap<String, String>,
    #[serde(default)]
    pub dependencies: DependencySection,
    #[serde(default)]
    pub ports:      PortSection,
    #[serde(default)]
    pub volumes:    VolumeSection,
    #[serde(default)]
    pub lifecycle:  LifecycleSection,
    #[serde(default)]
    pub services:   HashMap<String, ServiceDef>,
    pub health:     Option<HealthSection>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectSection {
    pub name:        String,
    pub version:     Option<String>,
    pub description: Option<String>,
    pub author:      Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ContainerSection {
    pub distro:      String,
    pub name:        Option<String>,
    #[serde(default = "default_true")]
    pub auto_start:  bool,
}

impl ContainerSection {
    pub fn effective_name(&self, project_name: &str) -> String {
        self.name.clone().unwrap_or_else(|| {
            project_name.replace(' ', "-").to_lowercase()
        })
    }
}

fn default_true() -> bool { true }

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DependencySection {
    #[serde(default)] pub apt:      Vec<String>,
    #[serde(default)] pub pacman:   Vec<String>,
    #[serde(default)] pub dnf:      Vec<String>,
    #[serde(default)] pub apk:      Vec<String>,
    #[serde(default)] pub pip:      Vec<String>,
    #[serde(default)] pub npm:      Vec<String>,
    #[serde(default)] pub cargo:    Vec<String>,
    #[serde(default)] pub gem:      Vec<String>,
    #[serde(default)] pub composer: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PortSection {
    #[serde(default)] pub expose: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct VolumeSection {
    #[serde(default)] pub mounts: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LifecycleSection {
    #[serde(default)] pub on_create: Vec<String>,
    #[serde(default)] pub on_start:  Vec<String>,
    #[serde(default)] pub on_stop:   Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServiceDef {
    pub command:     String,
    pub working_dir: Option<String>,
    #[serde(default = "default_true")]
    pub enabled:     bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthSection {
    pub command:  String,
    pub interval: Option<u32>,
    pub retries:  Option<u32>,
    pub timeout:  Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum MelParseError {
    #[error("File manifest tidak ditemukan: '{0}'")]
    NotFound(String),
    #[error("Gagal parse TOML: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Manifest tidak valid: {0}")]
    Invalid(String),
}

pub async fn load_mel_file(path: &str) -> Result<MelManifest, MelParseError> {
    if !std::path::Path::new(path).exists() {
        return Err(MelParseError::NotFound(path.to_string()));
    }
    let content = fs::read_to_string(path).await?;
    let manifest: MelManifest = toml::from_str(&content)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(m: &MelManifest) -> Result<(), MelParseError> {
    if m.project.name.trim().is_empty() {
        return Err(MelParseError::Invalid(
            "[project].name wajib diisi".into()
        ));
    }
    if m.container.distro.trim().is_empty() {
        return Err(MelParseError::Invalid(
            "[container].distro wajib diisi (lihat: melisa --search)".into()
        ));
    }
    for port in &m.ports.expose {
        if port.split(':').count() != 2 {
            return Err(MelParseError::Invalid(format!(
                "Format port salah '{}': harus 'host:container'", port
            )));
        }
    }
    for vol in &m.volumes.mounts {
        if vol.split(':').count() != 2 {
            return Err(MelParseError::Invalid(format!(
                "Format volume salah '{}': harus 'host_path:container_path'", vol
            )));
        }
    }
    Ok(())
}

pub fn validate_manifest_pub(m: &MelManifest) -> Result<(), MelParseError> {
    validate_manifest(m)
}