use std::env;
use crate::cli::color_text::{GREEN, BLUE, BOLD, RESET};

pub struct Prompt {
    pub user: String,
    pub home: String,
}

impl Prompt {
    pub fn new() -> Self {
        // Ambil nama user dari environment SSH/System
        let user = env::var("SUDO_USER")
            .or_else(|_| env::var("USER"))
            .or_else(|_| env::var("LOGNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        
        // Internal Melisa tetap mengacu ke /root
        let home = "/root".to_string(); 

        Self { user, home }
    }

    pub fn build(&self) -> String {
        let curr_path = env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(&self.home, "~");
        
        // Output: melisa@afira:~> atau melisa@saferoom:~>
        format!("{BOLD}{GREEN}melisa@{}{RESET}:{BLUE}{}{RESET}> ", self.user, curr_path)
    }
}