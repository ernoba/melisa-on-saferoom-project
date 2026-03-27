# Architecture & Internals

This section is for contributors, auditors, and curious developers who want to understand how MELISA works under the hood.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    MELISA HOST (Rust Binary)                │
│                                                             │
│  main.rs                                                    │
│  ├── check_root() → re-exec via sudo if not root            │
│  ├── Non-interactive mode (args provided): execute_command()│
│  └── Interactive mode: display_banner() → melisa() REPL     │
│                                                             │
│  cli/                                                       │
│  ├── melisa_cli.rs   ← Main REPL loop (rustyline)           │
│  ├── executor.rs     ← Command router & dispatcher          │
│  ├── helper.rs       ← Tab-completion & history             │
│  ├── prompt.rs       ← Dynamic prompt builder & history mgmt│
│  ├── loading.rs      ← Async spinner for long operations    │
│  ├── wellcome.rs     ← Boot animation & system dashboard    │
│  └── color_text.rs   ← ANSI color constants                 │
│                                                             │
│  core/                                                      │
│  ├── container.rs    ← LXC container CRUD & interaction     │
│  ├── metadata.rs     ← Container metadata injection & read  │
│  ├── setup.rs        ← Host initialization routine          │
│  ├── user_management.rs  ← User lifecycle & sudoers deploy  │
│  ├── project_management.rs ← Git project & sync operations  │
│  └── root_check.rs   ← Privilege verification helpers       │
│                                                             │
│  distros/                                                   │
│  ├── distro.rs       ← LXC distribution list fetch & cache  │
│  └── host_distro.rs  ← Host OS detection & firewall config  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                  MELISA CLIENT (Bash Scripts)               │
│                                                             │
│  melisa (entry point)                                       │
│  ├── Pre-flight: verify ssh, rsync, git                     │
│  ├── Module load: utils.sh, auth.sh, exec.sh (db.sh)        │
│  ├── init_auth()                                            │
│  └── Command routing → auth_*, exec_*, exec_forward         │
│                                                             │
│  auth.sh    ← Profile registry + SSH key management         │
│  exec.sh    ← Remote ops, project sync, file transfer       │
│  utils.sh   ← Logging, colors, SSH key generation           │
│  db.sh      ← Local project path registry                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Async Runtime

MELISA uses **Tokio** as its async runtime. The binary is compiled with `#[tokio::main]` and uses `tokio::process::Command` for all subprocess invocations instead of `std::process::Command`.

This is critical for two reasons:

1. **Non-blocking subprocess execution:** LXC operations (creating containers, waiting for network) can take minutes. Using `tokio::process::Command` with `.await` means the runtime can service other work while waiting.

2. **Async stdin reading:** Interactive commands that require user confirmation (like `--delete` and `--remove`) use `tokio::io::BufReader::new(io::stdin())` with `read_line().await` to avoid blocking the executor thread.

---

## Module Deep Dives

- **[Rust Server Internals](./rust-internals.md)** — The Tokio runtime, REPL design, command routing, LXC operations, and the loading spinner implementation.

- **[Bash Client Internals](./client-internals.md)** — The module sourcing architecture, SSH multiplexing, the project path registry, and the sync pipeline.