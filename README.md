<div align="center">

# MELISA

**Managed Environment for Linux Isolated Server Architecture**

*A jail-shell + LXC orchestration system for teams who want clean, isolated development servers — without the overhead of full virtualization.*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

---

## What is MELISA?

MELISA turns a bare Linux server into a **multi-user container lab**. Users SSH in and land directly in a controlled shell (not bash). They can create, start, stop, and enter LXC containers. They can collaborate on Git-backed projects. They cannot touch the host system, other users' files, or anything outside their permitted scope.

**The short version:**
- Server runs the MELISA binary (Rust), which is the login shell for all users
- Client uses a Bash script toolkit to sync code, run scripts, and forward commands from your workstation
- LXC provides the container isolation
- Git provides the project collaboration layer
- The `.mel` manifest file drives the Deployment Engine for automated container setup

---

## Quick Start

### 1. Install the Server

On your Linux server (physical console — SSH is blocked for setup):

```bash
# Clone and build
git clone https://github.com/ernoba/melisa-on-saferoom-project.git
cd melisa-on-saferoom-project
cargo build --release

# Install and initialize (must be on physical terminal, not SSH)
sudo -E ./target/release/melisa --setup
```

### 2. Install the Client

On your workstation:

```bash
curl -sSL https://raw.githubusercontent.com/ernoba/melisa-on-saferoom-project/main/src/melisa_client/install.sh | bash
```

### 3. Connect to Your Server

```bash
# Register the server (prompts for your MELISA username)
melisa auth add myserver root@192.168.1.100

# Switch to it as the active server
melisa auth switch myserver
```

### 4. Your First Container

```bash
# Search available distributions
melisa --search ubuntu

# Create a container
melisa --create mybox ubu-jammy-x64

# Start it
melisa --run mybox

# Enter it
melisa --use mybox
```

---

### 5. Add a User

```bash
# Create a new MELISA user (interactive: prompts for role + password)
melisa --add alice
```

Alice SSHes in and lands directly in the MELISA prompt. She can see and manage her containers. She cannot touch the host system, other users, or anything outside her permissions.

---

### 6. The Development Workflow

From your workstation:

```bash
# Clone a project from the server to your local machine
melisa clone myproject

# Make changes, then push everything to the server in one command
cd myproject
melisa sync

# Run a local script inside a remote container
melisa run mybox test_suite.sh

# Upload a build artifact to a container
melisa upload mybox ./dist/ /opt/myapp/
```

---

### 7. Deploy with a Manifest

```bash
# Write a .mel manifest (TOML format), then:
melisa --up ./myapp/program.mel

# Stop the deployment
melisa --down ./myapp/program.mel

# Inspect the manifest without deploying
melisa --mel-info ./myapp/program.mel
```

---

## Command Reference

### Server CLI (interactive shell)

> Commands marked **Admin** require administrator role.

| Command | Description |
|---------|-------------|
| `melisa --list` | List all LXC containers |
| `melisa --active` | List running containers only |
| `melisa --run <n>` | Start a container |
| `melisa --stop <n>` | Stop a container |
| `melisa --use <n>` | Attach an interactive shell to a container |
| `melisa --send <n> <cmd>` | Execute a command inside a container |
| `melisa --info <n>` | Display container metadata |
| `melisa --ip <n>` | Get the internal IP address of a container |
| `melisa --upload <n> <dest>` | Upload a tarball stream into a container |
| `melisa --projects` | List projects in your workspace |
| `melisa --update <project> [--force]` | Pull latest from master into your workspace |
| `melisa --up <file.mel>` | Deploy a project from a `.mel` manifest **(Admin)** |
| `melisa --down <file.mel>` | Stop a deployment defined in a `.mel` manifest **(Admin)** |
| `melisa --mel-info <file.mel>` | Display parsed info for a `.mel` manifest |
| `melisa --create <n> <code>` | Provision a new container **(Admin)** |
| `melisa --delete <n>` | Destroy a container — confirmation required **(Admin)** |
| `melisa --search <keyword>` | Search available LXC distributions **(Admin)** |
| `melisa --share <n> <host_path> <cont_path>` | Mount a host directory into a container **(Admin)** |
| `melisa --reshare <n> <host_path> <cont_path>` | Unmount a host directory from a container **(Admin)** |
| `melisa --add <user>` | Create a new MELISA user **(Admin)** |
| `melisa --remove <user>` | Delete a user **(Admin)** |
| `melisa --upgrade <user>` | Promote a user to Administrator **(Admin)** |
| `melisa --passwd <user>` | Change a user's password **(Admin)** |
| `melisa --user` | List all MELISA users **(Admin)** |
| `melisa --new_project <n>` | Create a shared project repository **(Admin)** |
| `melisa --invite <proj> <user...>` | Invite one or more users to a project **(Admin)** |
| `melisa --out <proj> <user...>` | Remove one or more users from a project **(Admin)** |
| `melisa --pull <user> <proj>` | Pull code from a user's workspace into the master repo **(Admin)** |
| `melisa --update-all <proj>` | Push master to all member workspaces **(Admin)** |
| `melisa --delete_project <n>` | Delete a project and all member copies — irreversible **(Admin)** |
| `melisa --setup` | Initialize host environment **(Admin, physical terminal only)** |
| `melisa --clear` | Purge the command history **(Admin)** |
| `melisa --clean` | Remove orphaned sudoers configuration files **(Admin)** |
| `melisa --version` | Print version information |

> **`--audit` flag:** Append `--audit` to any server command to disable the spinner and stream raw subprocess output directly to the terminal. Useful for debugging hangs or silent failures. Example: `melisa --create mybox ubu-jammy-x64 --audit`

### Client CLI (your workstation)

| Command | Description |
|---------|-------------|
| `melisa auth add <n> <user@ip>` | Register a server profile (prompts for MELISA username) |
| `melisa auth switch <n>` | Switch active server |
| `melisa auth list` | List registered servers |
| `melisa auth remove <n>` | Remove a server profile |
| `melisa clone <project> [--force]` | Clone a project from the server |
| `melisa sync` | Push local changes to the server |
| `melisa get <project> [--force]` | Pull server-side changes to local |
| `melisa run <container> <file>` | Execute a local script in a remote container |
| `melisa run-tty <container> <file>` | Execute interactively (TTY) |
| `melisa upload <container> <dir> <dest>` | Upload a directory into a container |
| `melisa tunnel <container> <remote_port> [local_port]` | Open an SSH tunnel to a container port |
| `melisa tunnel-list` | List all active tunnels |
| `melisa tunnel-stop <container> [remote_port]` | Stop an active tunnel |
| `melisa shell` | Open SSH shell to the MELISA host |

---

## Deployment Engine (`.mel` Manifests)

The Deployment Engine lets you describe an entire container environment in a single TOML file — the `.mel` manifest. Running `melisa --up` provisions the container, installs all dependencies, configures volumes and environment variables, and runs lifecycle hooks automatically.

### Minimal Example

```toml
[project]
name    = "myapp"
version = "1.0"

[container]
distro = "ubu-jammy-x64"
```

### Full Manifest Reference

```toml
# ── PROJECT ──────────────────────────────────────────────────────────────
[project]
name        = "myapp"
version     = "1.0"
description = "My web application"
author      = "alice"

# ── CONTAINER ────────────────────────────────────────────────────────────
[container]
distro     = "ubu-jammy-x64"      # Distribution code (from melisa --search)
name       = "myapp-prod"          # Optional: overrides the default (project name)
auto_start = true                  # Start the container after creation (default: true)

# ── ENVIRONMENT VARIABLES ────────────────────────────────────────────────
# Injected into /etc/environment inside the container
[env]
APP_PORT = "8080"
APP_ENV  = "production"
DB_HOST  = "10.0.3.5"

# ── DEPENDENCIES ─────────────────────────────────────────────────────────
# Specify packages per package manager; MELISA auto-detects which one is
# available inside the container.
[dependencies]
apt     = ["python3", "python3-pip", "nginx"]   # Debian/Ubuntu
pacman  = ["python", "python-pip", "nginx"]     # Arch Linux
dnf     = ["python3", "python3-pip", "nginx"]   # Fedora/RHEL
apk     = ["python3", "py3-pip", "nginx"]       # Alpine
zypper  = ["python3", "python3-pip", "nginx"]   # openSUSE

# Language-level package managers (installed on top of system deps)
pip      = ["flask", "gunicorn", "requests"]
npm      = ["pm2", "yarn"]
cargo    = ["ripgrep"]
gem      = ["bundler", "rails"]
composer = ["laravel/framework"]

# ── VOLUMES ──────────────────────────────────────────────────────────────
# Bind mounts: "host_path:container_path"
[volumes]
mounts = [
    "/home/alice/myapp:/app",
    "/var/data:/data",
]

# ── PORTS ────────────────────────────────────────────────────────────────
# Informational — use `melisa tunnel` to expose these to your workstation
[ports]
expose = ["8080", "5432"]

# ── LIFECYCLE HOOKS ──────────────────────────────────────────────────────
# Shell commands executed inside the container at specific lifecycle events
[lifecycle]
on_create = [
    "mkdir -p /app/logs",
    "chown -R www-data:www-data /app",
    "pip3 install -r /app/requirements.txt",
]
on_stop = [
    "nginx -s stop",
]

# ── SERVICES ─────────────────────────────────────────────────────────────
# Named service definitions (informational; use melisa --send to run them)
[services]
web    = { command = "gunicorn -w 4 app:app", working_dir = "/app", enabled = true }
worker = { command = "python3 worker.py",     working_dir = "/app", enabled = false }

# ── HEALTH CHECK ─────────────────────────────────────────────────────────
[health]
command  = "curl -sf http://localhost:8080/health"
interval = 5    # seconds between retries
retries  = 3
timeout  = 10
```

### Deployment Steps

When `melisa --up` runs, the engine executes seven ordered steps:

```
[STEP 1/7]  Provision the container (or start it if it already exists)
[STEP 2/7]  Detect the container's package manager (apt, pacman, dnf, apk…)
[STEP 3/7]  Install system dependencies
[STEP 4/7]  Install language-level dependencies (pip, npm, cargo, gem, composer)
[STEP 5/7]  Configure volumes (restart container if mounts are added)
[STEP 6/7]  Inject environment variables into /etc/environment
[STEP 7/7]  Run lifecycle hooks (on_create)
            Run health check (if defined)
```

---

## SSH Tunnels

`melisa tunnel` creates a persistent SSH tunnel from your workstation to a port inside a remote container — with zero manual SSH configuration. This works across different networks as long as the server's SSH port is reachable.

```bash
# Forward container port 8080 to localhost:8080
melisa tunnel myapp 8080

# Forward to a different local port (if 8080 is already in use)
melisa tunnel myapp 8080 9090

# Check all active tunnels
melisa tunnel-list

# Stop a specific tunnel
melisa tunnel-stop myapp 8080
```

Tunnel state is stored in `~/.config/melisa/tunnels/`. If a previous tunnel for the same container+port exists, it is stopped automatically before the new one starts.

---

## Architecture

MELISA is split into two parts:

**Server (Rust binary — `src/`)**
```
src/
├── main.rs                   ← Entry point, Tokio async runtime, privilege escalation
├── cli/
│   ├── melisa_cli.rs         ← REPL loop (rustyline)
│   ├── executor.rs           ← Command router & dispatcher
│   ├── helper.rs             ← Tab-completion & history hints
│   ├── prompt.rs             ← Dynamic prompt builder
│   ├── loading.rs            ← Async spinner for long operations
│   ├── wellcome.rs           ← Boot animation & system dashboard
│   └── color_text.rs         ← ANSI color constants
├── core/
│   ├── container.rs          ← LXC CRUD & container interaction
│   ├── metadata.rs           ← Container metadata injection (atomic write)
│   ├── setup.rs              ← Host initialization (multi-distro)
│   ├── user_management.rs    ← User lifecycle & sudoers deployment
│   ├── project_management.rs ← Git project & sync operations
│   └── root_check.rs         ← Privilege verification
├── deployment/
│   ├── mel_parser.rs         ← .mel manifest parser (TOML → typed structs)
│   ├── deployer.rs           ← Deployment Engine (--up / --down / --mel-info)
│   ├── dependency.rs         ← System & language dependency installer
│   └── tests.rs              ← Integration tests for the deployment pipeline
└── distros/
    ├── distro.rs             ← LXC distribution list fetch & cache
    └── host_distro.rs        ← Host OS detection & firewall config
```

**Client (Bash scripts — `src/melisa_client/`)**
```
src/melisa_client/
├── src/
│   ├── melisa            ← Entry point (sources all modules)
│   ├── auth.sh           ← SSH profile & multiplexing management
│   ├── exec.sh           ← Remote execution, project sync, tunnels, file transfer
│   ├── utils.sh          ← Logging, colors, SSH key generation
│   └── db.sh             ← Local project path registry
├── ut_/
│   ├── test_melisa.py                 ← Client unit & integration tests
│   └── test_tunnel_and_crossregion.py ← Tunnel & cross-region tests
└── install.sh            ← Client installer
```

---

## Security Model

MELISA uses seven layers of security:

1. **Physical Handshake** — `--setup` refuses to run over SSH
2. **Jail Shell** — Users land in MELISA, not bash
3. **SUID Binary + Sudoers** — Controlled privilege escalation only for MELISA-managed operations
4. **Surgical Sudoers Policies** — Per-user whitelist of exactly the binaries each role needs
5. **Home Directory Isolation** — `chmod 711 /home`, `chmod 700 /home/<user>`
6. **LXC Namespace Isolation** — Container root (UID 0) maps to unprivileged UID 100000 on the host
7. **History Security** — TOCTOU-safe purge, `0600` permissions on history files

See [Security Model](doc/src/security.md) for full details.

---

## Contributing

Contributions are welcome — bug reports, feature requests, documentation fixes, and code improvements.

### Repository Structure

```
melisa-on-saferoom-project/
├── src/              ← Rust server source
├── doc/
│   ├── src/          ← MDBook Markdown documentation
│   └── book.toml     ← MDBook configuration
├── Cargo.toml
├── LICENSE
└── README.md
```

### Development Setup

**Building the server:**

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/ernoba/melisa-on-saferoom-project.git
cd melisa-on-saferoom-project
cargo build

# Run (requires root on Linux)
sudo -E ./target/debug/melisa
```

**Building the documentation:**

```bash
cargo install mdbook
cd doc
mdbook serve   # Live preview at http://localhost:3000
mdbook build   # Build static HTML to doc/book/
```

**Running the client tests:**

```bash
cd src/melisa_client/ut_
python3 test_melisa.py
python3 test_tunnel_and_crossregion.py
```

### How to Contribute

1. **Fork** the repository
2. **Create a branch** for your change: `git checkout -b feature/my-feature`
3. **Make your changes** and ensure the build passes: `cargo build`
4. **Commit** with a clear message: `git commit -m "feat: add --freeze command for container pause"`
5. **Push** and open a **Pull Request** against `main`

### What We're Looking For

- Bug fixes with a test case or reproduction steps
- New distribution support in `src/distros/host_distro.rs`
- Documentation improvements (especially in `doc/src/`)
- New story chapters in `doc/src/story/`
- Performance improvements to the Tokio async pipeline
- Client-side test coverage in `src/melisa_client/ut_/`

### Code Style

- Rust: standard `rustfmt` formatting (`cargo fmt`)
- Bash: POSIX-compatible where possible; document non-POSIX constructs
- Documentation: MDBook Markdown; match the existing tone in `doc/src/`

### Reporting Bugs

Open a GitHub Issue with:
- Host OS and version
- MELISA version (`melisa --version`)
- The exact command that failed
- The full output including any error messages
- Expected vs. actual behavior

---

## License

MIT License — Copyright (c) 2026 Erick Adriano Sebastian

See [LICENSE](./LICENSE) for the full text.

---

### Join Community
[Telegram melisa](https://t.me/melisaproject) | [Instagram melisa](https://www.instagram.com/melisa.project)

*Built to end the late Friday night "I think I broke something" messages.*