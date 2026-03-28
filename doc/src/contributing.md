# Contributing

MELISA is open-source software under the MIT License. Contributions — bug reports, feature ideas, documentation improvements, and code — are welcome.

---

## Repository Structure

```
melisa-on-saferoom-project/
├── src/
│   ├── cli/              ← Terminal UI (REPL, prompts, colors, spinner)
│   ├── core/             ← Business logic (containers, users, projects, setup)
│   ├── deployment/       ← Deployment Engine (mel_parser, deployer, dependency)
│   ├── distros/          ← OS detection, LXC distribution catalog
│   ├── melisa_client/    ← Bash client scripts and installer
│   │   ├── src/          ← Client source (melisa, auth.sh, exec.sh, utils.sh, db.sh)
│   │   ├── ut_/          ← Client tests (test_melisa.py, test_tunnel_and_crossregion.py)
│   │   └── install.sh
│   └── main.rs           ← Entry point, Tokio runtime
├── doc/
│   ├── src/              ← MDBook Markdown source files
│   └── book.toml         ← MDBook configuration
├── Cargo.toml
├── LICENSE
└── README.md
```

---

## Development Setup

### Building the Server

```bash
# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/ernoba/melisa-on-saferoom-project.git
cd melisa-on-saferoom-project
cargo build

# Run (requires root on Linux)
sudo -E ./target/debug/melisa
```

### Building the Documentation

```bash
# Install mdBook
cargo install mdbook

# Build the book
cd doc
mdbook build

# Serve locally with hot-reload
mdbook serve --open
```

### Running the Client Tests

```bash
cd src/melisa_client/ut_
python3 test_melisa.py
python3 test_tunnel_and_crossregion.py
```

---

## Coding Standards

### Rust (Server)

- All subprocess calls must use `tokio::process::Command` — **never** `std::process::Command` for operations that could block. The Tokio runtime must not be starved.
- All file I/O should use `tokio::fs` — use the synchronous `std::fs` only when immediately after a synchronous operation (like after `rustyline::save_history` to set file permissions).
- Error handling: prefer `thiserror` for typed errors in library functions; use `eprintln!` with the appropriate color constants for user-facing errors.
- Security-critical paths must validate against path traversal (check for `/`, `\\`, `..` in container names and file paths).
- Write atomically: temp file → set permissions → `fsync` → rename.

### Bash (Client)

- Always use `set -o pipefail` in scripts that use pipelines.
- Route all error and warning messages to stderr (`>&2`) to avoid corrupting pipeline outputs.
- Use `realpath` with a fallback (`2>/dev/null || echo "$path"`) when resolving paths.
- Prefer POSIX-compatible constructs when possible (`grep | mv` over `sed -i`).
- All `log_error` and `log_warning` calls must use `>&2`.

---

## Adding a New Server Command

1. Add the command handler in `src/cli/executor.rs` inside the `match sub_cmd` block:

```rust
"--my_command" => {
    if let Some(arg) = parts.get(2) {
        my_function(arg).await;
    } else {
        println!("{}[ERROR]{} Usage: melisa --my_command <arg>{}", RED, BOLD, RESET);
    }
},
```

2. Implement the function in the appropriate `src/core/*.rs` module.

3. Add the command to the `--help` output in `executor.rs`.

4. Add documentation in `doc/src/server-cli/`.

---

## Adding a New Client Command

1. Add the routing case in `src/melisa_client/src/melisa`:

```bash
my_command)
    if [ -z "$1" ]; then
        log_error "Usage: melisa my_command <arg>"
        exit 1
    fi
    exec_my_command "$1"
    ;;
```

2. Implement `exec_my_command()` in `src/melisa_client/src/exec.sh`.

3. Add the command to the help block in the `melisa` entry point.

4. Add documentation in `doc/src/client-cli/`.

---

## Submitting Changes

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes with tests if applicable
4. Run `cargo clippy` and `cargo fmt` before committing
5. Open a Pull Request describing what you changed and why

---

## What We're Looking For

- Bug fixes with a test case or reproduction steps
- New distribution support in `src/distros/host_distro.rs`
- Documentation improvements (especially in `doc/src/`)
- New story chapters in `doc/src/story/`
- Performance improvements to the Tokio async pipeline
- Client-side test coverage in `src/melisa_client/ut_/`
- New `.mel` manifest examples and deployment patterns

---

## License

MELISA is released under the **MIT License**. See [LICENSE](https://github.com/ernoba/melisa-on-saferoom-project/blob/main/LICENSE) for the full text.

By contributing, you agree that your contributions will be licensed under the same MIT License.