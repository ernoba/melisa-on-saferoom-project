mod client;
mod cli;
mod local_data;

use client::login::login;
use cli::setup_lxc::check_lxc;
use cli::setup_lxc::check_root;
use cli::install_lxc::install;
use cli::melisa_cli::melisa;
use cli::wellcome::display_melisa_banner;

fn main() {
    display_melisa_banner();
    if login("user", "pass") {
        if check_root() {
            if check_lxc() {
                melisa();
            } else {
                println!("LXC is not installed. Installing now...");
                install();
            }
        } else {
            println!("This program must be run as root. Please run with sudo.");
            std::process::exit(1);
        }
    }
}