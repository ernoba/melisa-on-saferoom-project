use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

// Fungsi ini menerima pesan dan sebuah 'closure' (tugas yang ingin dijalankan)
pub fn execute_with_spinner<F, T>(message: &str, action: F) -> T 
where 
    F: FnOnce() -> T 
{
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));

    // Jalankan tugasnya
    let result = action();

    // Hentikan spinner
    pb.finish_and_clear();
    
    result
}