# Client CLI Reference

The MELISA client is a modular Bash script installed on your **local workstation**. It communicates with the MELISA host exclusively over SSH, providing a unified interface for remote container management, project synchronization, file transfer, and port tunneling.

---

## Architecture

```
~/.local/bin/melisa          ← Entry point (command router)
~/.local/share/melisa/
├── utils.sh                 ← Color/logging helpers, SSH key management
├── auth.sh                  ← Profile management (add/switch/list/remove)
├── exec.sh                  ← Remote execution engine (run/upload/clone/sync/get/tunnel)
└── db.sh                    ← Local project path registry
~/.config/melisa/
├── profiles.conf            ← Server profile registry (name=user@host|melisa_user)
├── active                   ← Name of currently active server
├── registry                 ← Local project paths (name|/absolute/path)
└── tunnels/                 ← Active tunnel state (.meta and .pid files)
```

All four modules are sourced by the entry point at startup. The `auth` subsystem is initialized before any command is dispatched.

---

## Module Responsibilities

### `melisa` (entry point)
- Pre-flight SSH dependency check
- Module integrity verification (aborts if any `.sh` is missing)
- Command routing to sub-functions
- Fallback: forwards unrecognized commands to the active server via SSH

### `utils.sh`
- ANSI color constants (`BOLD`, `GREEN`, `RED`, `CYAN`, `YELLOW`, `RESET`)
- Standardized log functions: `log_info`, `log_success`, `log_warning`, `log_error`
- `ensure_ssh_key` — generates Ed25519 keypair if no SSH identity exists

### `auth.sh`
- `init_auth` — creates required directories and files
- `auth_add` — register server, copy SSH key, configure multiplexing
- `auth_switch` — change active server
- `auth_list` — display all profiles with active marker
- `auth_remove` — delete a profile
- `get_active_conn` — resolve SSH connection string (used internally by exec.sh)
- `get_remote_user` — resolve MELISA username on the server
- `get_active_melisa_user` — MELISA username with fallback to SSH user

### `exec.sh`
- `exec_run` — stream a local script to a remote container interpreter
- `exec_run_tty` — upload + execute interactively + cleanup
- `exec_upload` — tar stream to remote container
- `exec_clone` — git clone or rsync from server to local
- `exec_sync` — git push + .env rsync (post-receive hook handles server update)
- `exec_get` — rsync pull from server workspace to local
- `exec_tunnel` — open a persistent SSH tunnel to a container port
- `exec_tunnel_list` — list all active tunnels with status
- `exec_tunnel_stop` — terminate an active tunnel and clean up state files
- `exec_forward` — SSH forward for unrecognized commands

### `db.sh`
- `db_update_project` — register/update a project path mapping
- `db_get_path` — retrieve a project's local path by name
- `db_identify_by_pwd` — detect current project from working directory

---

## Command Summary

| Command | Module | Description |
|---------|--------|-------------|
| `auth add <n> <user@ip>` | auth.sh | Register a new server |
| `auth switch <n>` | auth.sh | Switch active server |
| `auth list` | auth.sh | List all servers |
| `auth remove <n>` | auth.sh | Unregister a server |
| `clone <n> [--force]` | exec.sh | Clone project workspace |
| `sync` | exec.sh | Push changes to server |
| `get <n> [--force]` | exec.sh | Pull data from server |
| `run <container> <file>` | exec.sh | Execute script remotely |
| `run-tty <container> <file>` | exec.sh | Execute interactively |
| `upload <cont> <dir> <dest>` | exec.sh | Transfer directory |
| `tunnel <cont> <r_port> [l_port]` | exec.sh | Open SSH tunnel to container port |
| `tunnel-list` | exec.sh | List active tunnels |
| `tunnel-stop <cont> [r_port]` | exec.sh | Stop an active tunnel |
| `shell` | melisa | Open SSH shell to host |
| `<any other command>` | exec.sh | Forward to MELISA server |