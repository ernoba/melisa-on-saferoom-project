use crate::VERSION;
use crate::AUTHORS;

pub async fn print_version() {
    println!("MELISA Engine v{}", VERSION);
    println!("Copyright (c) 2026 {}", AUTHORS);
}

