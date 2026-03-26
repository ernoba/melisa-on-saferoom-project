# MELISA 🦀
### **Management Environment Linux Sandbox**
*Performance-focused, lightweight, and secure isolated development environments — powered by Rust.*

---

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Platform](https://img.shields.io/badge/platform-Linux-lightgrey.svg)](https://kernel.org)
[![LXC](https://img.shields.io/badge/containers-LXC-blue.svg)](https://linuxcontainers.org/)
[![Version](https://img.shields.io/badge/version-0.1.2--delta-cyan.svg)](./Cargo.toml)

---

## What is MELISA?

MELISA is a **server–client container management system** built on top of Linux Containers (LXC). It lets you provision isolated development environments on a Linux server, manage multi-user access with surgical `sudoers` policies, and synchronize code from your laptop to your container over SSH — all from a single CLI.

It was built to solve a specific problem: developers breaking their local machines and losing hours to environment recovery. With MELISA, the environment lives on a server. Developers get a clean room. If it breaks, it's a two-minute `--delete` and `--create`.

```
┌────────────────────────────┐         ┌──────────────────────────┐
│    MELISA HOST (Server)    │         │  WORKSTATION (Client)    │
│                            │         │                          │
│  ┌──────────────────────┐  │   SSH   │  ┌────────────────────┐  │
│  │  melisa (Rust binary)│  │◄───────►│  │  melisa (Bash CLI) │  │
│  └──────────────────────┘  │         │  └────────────────────┘  │
│  ┌──────────────────────┐  │         └──────────────────────────┘
│  │   LXC Containers     │  │
│  │  ┌──┐  ┌──┐  ┌──┐    │  │
│  │  │C1│  │C2│  │C3│    │  │
│  │  └──┘  └──┘  └──┘    │  │
│  └──────────────────────┘  │
└────────────────────────────┘
```

---

## Key Features

- **Near-native performance** — LXC containers share the host kernel, no hypervisor overhead
- **Multi-distro server support** — Runs on Fedora, Ubuntu, Debian, Arch Linux (auto-detected at setup)
- **Jail shell** — Users SSH in and land directly in the MELISA prompt; no bash access
- **Surgical sudoers policies** — Per-user, whitelist-only privilege escalation
- **Git-backed project collaboration** — Push code from your laptop, server propagates to all team members automatically
- **SSH multiplexing** — Persistent connections make remote commands nearly instantaneous
- **One-command sync** — `melisa sync` stages, commits, pushes, and triggers server-side update in ~2 seconds
- **Async Rust core** — Tokio runtime; non-blocking LXC operations with live loading spinners

---

## Supported Platforms

| Host OS | Package Manager | Firewall |
|---------|-----------------|----------|
| Fedora, RHEL, CentOS, Rocky Linux | `dnf` | `firewalld` |
| Ubuntu | `apt-get` | `ufw` |
| Debian | `apt-get` | `ufw` |
| Arch Linux | `pacman` | `iptables` |
| Other | `apt-get` (fallback) | `ufw` (fallback) |

**Containers** can run any distribution available from the LXC image servers: Ubuntu, Debian, Fedora, Alpine, Arch, Kali, openSUSE, and more.

---

## Get Started

### Prerequisites

**Server:**
- A Linux machine (Fedora, Ubuntu, Debian, or Arch recommended)
- Physical/console access (required once for `--setup`)
- Rust toolchain: [rustup.rs](https://rustup.rs)

**Client (your workstation):**
- Any OS with `ssh`, `rsync`, and `git`

---

### 1. Install the Server

```bash
# Install Rust if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/ernoba/melisa-on-saferoom-project.git
cd melisa-on-saferoom-project
cargo build

# First launch (requires physical terminal — not SSH)
sudo -E ./target/debug/melisa
```

Once the MELISA prompt appears, run the one-time setup:

```
melisa@yourhostname:~> melisa --setup
```

`--setup` detects your host OS and automatically installs LXC, configures the network bridge, deploys the binary to `/usr/local/bin/melisa`, registers it as a login shell, sets up sudoers rules, and hardens the system. ~15 steps, each with timeout protection.

> **Security note:** `--setup` requires a physical terminal and refuses SSH connections by design. An attacker who compromises your network before setup completes should not be able to remotely bootstrap your security infrastructure.

---

### 2. Install the Client

On your workstation (laptop, desktop, CI runner):

```bash
cd melisa-on-saferoom-project/src/melisa_client
./install.sh
```

The installer deploys the Bash client to `~/.local/bin/melisa`, registers it in your `$PATH`, and sets up the config directory at `~/.config/melisa/`.

Register your server:

```bash
melisa auth add myserver root@<server-ip>
```

This generates an SSH key if you don't have one, copies it to the server, configures SSH multiplexing, and saves the profile.

Test the connection:

```bash
melisa --list
```

---

### 3. Create Your First Container

```bash
# On the server — search for a distribution
melisa --search ubuntu

# Provision a container
melisa --create mybox ubu-jammy-x64

# Start it
melisa --run mybox

# Enter it
melisa --use mybox
```

---

### 4. Add a User

```bash
# Create a new MELISA user (interactive: prompts for role + password)
melisa --add alice
```

Alice SSHes in and lands directly in the MELISA prompt. She can see and manage her containers. She cannot touch the host system, other users, or anything outside her permissions.

---

### 5. The Development Workflow

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

## Command Reference

### Server CLI (interactive shell)

| Command | Description |
|---------|-------------|
| `melisa --list` | List all LXC containers |
| `melisa --active` | List running containers only |
| `melisa --create <n> <code>` | Provision a new container |
| `melisa --run <n>` | Start a container |
| `melisa --stop <n>` | Stop a container |
| `melisa --use <n>` | Attach an interactive shell to a container |
| `melisa --send <n> <cmd>` | Execute a command inside a container |
| `melisa --info <n>` | Display container metadata |
| `melisa --delete <n>` | Destroy a container (confirmation required) |
| `melisa --search <keyword>` | Search available LXC distributions |
| `melisa --add <user>` | Create a new MELISA user (Admin) |
| `melisa --remove <user>` | Delete a user (Admin) |
| `melisa --new_project <n>` | Create a shared project (Admin) |
| `melisa --invite <proj> <users>` | Invite users to a project (Admin) |
| `melisa --projects` | List your projects |
| `melisa --update <project>` | Pull latest from master into your workspace |
| `melisa --setup` | Initialize host environment (Admin, physical terminal) |
| `melisa --version` | Print version information |

### Client CLI (your workstation)

| Command | Description |
|---------|-------------|
| `melisa auth add <n> <user@ip>` | Register a server profile |
| `melisa auth switch <n>` | Switch active server |
| `melisa auth list` | List registered servers |
| `melisa auth remove <n>` | Remove a server profile |
| `melisa clone <project>` | Clone a project from the server |
| `melisa sync` | Push local changes to the server |
| `melisa get <project>` | Pull server-side changes to local |
| `melisa run <container> <file>` | Execute a local script in a remote container |
| `melisa run-tty <container> <file>` | Execute interactively (TTY) |
| `melisa upload <container> <dir> <dest>` | Upload a directory into a container |
| `melisa shell` | Open SSH shell to the MELISA host |

---

## Architecture

MELISA is split into two parts:

**Server (Rust binary — `src/`)**
```
src/
├── main.rs               ← Entry point, Tokio async runtime, privilege escalation
├── cli/
│   ├── melisa_cli.rs     ← REPL loop (rustyline)
│   ├── executor.rs       ← Command router & dispatcher
│   ├── helper.rs         ← Tab-completion & history hints
│   ├── prompt.rs         ← Dynamic prompt builder
│   ├── loading.rs        ← Async spinner for long operations
│   ├── wellcome.rs       ← Boot animation & system dashboard
│   └── color_text.rs     ← ANSI color constants
├── core/
│   ├── container.rs      ← LXC CRUD & container interaction
│   ├── metadata.rs       ← Container metadata injection (atomic write)
│   ├── setup.rs          ← Host initialization (multi-distro)
│   ├── user_management.rs← User lifecycle & sudoers deployment
│   ├── project_management.rs ← Git project & sync operations
│   └── root_check.rs     ← Privilege verification
└── distros/
    ├── distro.rs         ← LXC distribution list fetch & cache
    └── host_distro.rs    ← Host OS detection & firewall config
```

**Client (Bash scripts — `src/melisa_client/`)**
```
src/melisa_client/
├── src/
│   ├── melisa            ← Entry point (sources all modules)
│   ├── auth.sh           ← SSH profile & multiplexing management
│   ├── exec.sh           ← Remote execution & project sync
│   ├── utils.sh          ← Logging, colors, SSH key generation
│   └── db.sh             ← Local project path registry
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
python3 _test_.py
python3 _test_2.py
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

###  Join Community
[Telegram melisa](https://t.me/melisaproject) | [Instagram melisa](https://www.instagram.com/melisa.project)

*Built to end the late Friday night "I think I broke something" messages.*
