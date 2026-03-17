use tokio::fs; // Gunakan tokio::fs untuk operasi async
use rustyline::{Editor, Config};
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::completion::FilenameCompleter;
use rustyline::highlight::MatchingBracketHighlighter;
use rustyline::hint::HistoryHinter;
use rustyline::validate::MatchingBracketValidator;

use crate::cli::color_text::{BOLD, RESET};
use crate::cli::helper::MelisaHelper;
use crate::cli::prompt::Prompt;
use crate::cli::executor::{execute_command, ExecResult};

// 1. Ubah menjadi pub async fn
pub async fn melisa() {
    // 2. Operasi folder secara async
    let _ = fs::create_dir_all("data").await; 
    let history_path = "data/history.txt";

    let config = Config::builder()
        .history_ignore_dups(true).ok()
        .map(|b| b.build())
        .unwrap_or_default();

    // Rustyline sendiri masih sinkron, tapi kita menjalankannya di dalam context async
    let mut rl: Editor<MelisaHelper, FileHistory> = Editor::with_config(config).expect("Fail init");

    rl.set_helper(Some(MelisaHelper {
        hinter: HistoryHinter {},
        highlighter: MatchingBracketHighlighter::new(),
        validator: MatchingBracketValidator::new(),
        file_completer: FilenameCompleter::new(),
    }));

    let _ = rl.load_history(history_path);
    let p_info = Prompt::new();

    println!("{BOLD}Authenticated as melisa. Access granted.{RESET}");

    loop {
        let prompt_str: String = p_info.build();

        // Note: rl.readline tetap memblokir thread saat menunggu input.
        // Dalam aplikasi CLI tunggal seperti Melisa, ini tidak masalah.
        match rl.readline(&prompt_str) {
            Ok(line) => {
                let input: &str = line.trim();
                
                if input.is_empty() { continue; }

                // 3. Await execute_command
                // Pastikan fungsi execute_command di executor.rs juga sudah 'async'
                match execute_command(input, &p_info.user, &p_info.home).await {
                    ExecResult::Break => {
                        let _ = rl.save_history(history_path);
                        break;
                    },
                    ExecResult::Error(e) => eprintln!("{}", e),
                    ExecResult::Continue => {
                        let _ = rl.add_history_entry(input);
                        let _ = rl.save_history(history_path); 
                    }
                }
            },
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => {
                let _ = rl.save_history(history_path);
                break;
            },
            _ => break,
        }
    }
}