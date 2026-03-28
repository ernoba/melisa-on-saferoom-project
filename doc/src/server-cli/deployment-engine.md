# Deployment Engine (`.mel` Manifests)

The MELISA Deployment Engine lets you describe an entire container environment in a single TOML file — the **`.mel` manifest**. One command provisions the container, installs all dependencies, configures volumes and environment variables, and runs lifecycle hooks automatically.

---

## Commands

### `--up <file.mel>`

**Access:** Administrator only

Reads a `.mel` manifest and deploys the described environment. If the container doesn't exist yet, it is created. If it exists but is stopped, it is started.

```
melisa@host:~> melisa --up ./myapp/program.mel
melisa@host:~> melisa --up /opt/projects/backend/backend.mel
```

**Output example:**

```
━━━ MELISA DEPLOYMENT ENGINE ━━━
[UP] Membaca manifest: ./myapp/program.mel

  Proyek   : myapp v1.0
  Kontainer: myapp
  Distro   : ubuntu-jammy-amd64
  Deps     : 7 paket total
  Volumes  : 1
  Ports    : 1

[STEP 1/7] Provisioning kontainer baru...
[STEP 2/7] Mendeteksi lingkungan kontainer...
[INFO] Package manager terdeteksi: apt-get
[STEP 3/7] Menginstall system dependencies...
[STEP 4/7] Menginstall language dependencies...
[STEP 5/7] Mengatur volumes...
[STEP 6/7] Menginjeksi environment variables...
[STEP 7/7] Menjalankan lifecycle hooks...
[HEALTH] Menjalankan health check...

━━━ DEPLOYMENT SELESAI ━━━
[OK] Kontainer 'myapp' berhasil di-deploy!
```

**Deployment steps (in order):**

| Step | Action |
|------|--------|
| 1/7 | Provision the container, or start it if it already exists |
| 2/7 | Auto-detect the package manager inside the container (`apt-get`, `pacman`, `dnf`, `apk`, `zypper`) |
| 3/7 | Install system-level dependencies (`[dependencies]` section, matching detected package manager) |
| 4/7 | Install language-level dependencies (`pip`, `npm`, `cargo`, `gem`, `composer`) |
| 5/7 | Configure volumes — restarts the container if any mount was added |
| 6/7 | Inject environment variables into `/etc/environment` inside the container |
| 7/7 | Run lifecycle hooks (`on_create`) |
| — | Run health check if `[health]` is defined |

---

### `--down <file.mel>`

**Access:** Administrator only

Reads a `.mel` manifest, runs the `on_stop` lifecycle hooks if defined, then stops the container.

```
melisa@host:~> melisa --down ./myapp/program.mel
```

If the container is already stopped, the command exits cleanly with an informational message — it is not an error.

---

### `--mel-info <file.mel>`

**Access:** All users

Parses and displays a summary of a `.mel` manifest **without** running any deployment. Useful for inspecting what a manifest will do before committing to `--up`.

```
melisa@host:~> melisa --mel-info ./myapp/program.mel
```

```
━━━ MELISA MANIFEST INFO ━━━
  Proyek   : myapp v1.0
  Kontainer: myapp  [STOPPED]
  Distro   : ubuntu-jammy-amd64
  ...
```

---

## The `.mel` Manifest Format

Manifests are standard **TOML** files. The recommended extension is `.mel`, and the convention is to place the file in the project root as `program.mel`.

### Minimal Manifest

The only required sections are `[project]` (with `name`) and `[container]` (with `distro`):

```toml
[project]
name = "myapp"

[container]
distro = "ubuntu-jammy-amd64"
```

### Full Manifest Reference

```toml
# ── PROJECT ──────────────────────────────────────────────────────────────
[project]
name        = "myapp"           # Required. Used as the default container name.
version     = "1.0"             # Optional. Informational only.
description = "My web app"      # Optional.
author      = "alice"           # Optional.

# ── CONTAINER ────────────────────────────────────────────────────────────
[container]
distro     = "ubuntu-jammy-amd64"  # Required. Use melisa --search to find codes.
name       = "myapp-prod"           # Optional. Overrides the default (project name).
                                    # Default: project name, lowercased, spaces → hyphens.
auto_start = true                   # Optional. Start container after creation. Default: true.

# ── ENVIRONMENT VARIABLES ────────────────────────────────────────────────
# Injected into /etc/environment inside the container via sed.
# The injection command is idempotent: the key is removed then re-added.
[env]
APP_PORT = "8080"
APP_ENV  = "production"
DB_HOST  = "10.0.3.5"

# ── DEPENDENCIES ─────────────────────────────────────────────────────────
# System packages: specify per package manager. MELISA auto-detects which
# manager is present inside the container and installs from the matching list.
# If the container has apt-get but only 'pacman' is specified, system deps are skipped.
[dependencies]
apt     = ["python3", "python3-pip", "nginx"]   # Debian / Ubuntu
pacman  = ["python", "python-pip", "nginx"]     # Arch Linux
dnf     = ["python3", "python3-pip", "nginx"]   # Fedora / RHEL
apk     = ["python3", "py3-pip", "nginx"]       # Alpine Linux
zypper  = ["python3", "python3-pip", "nginx"]   # openSUSE

# Language-level managers (run after system deps, regardless of package manager):
pip      = ["flask", "gunicorn", "requests"]    # pip3 install --break-system-packages
npm      = ["pm2", "yarn"]                      # npm install -g
cargo    = ["ripgrep"]                          # cargo install (one by one)
gem      = ["bundler", "rails"]                 # gem install
composer = ["laravel/framework"]                # composer global require

# ── VOLUMES ──────────────────────────────────────────────────────────────
# Bind mounts added to the container's LXC config.
# Format: "host_absolute_path:container_path"
# Ownership is automatically set to 100000:100000 (unprivileged UID mapping).
# A container restart is triggered automatically if any mounts are added.
[volumes]
mounts = [
    "/home/alice/myapp:/app",
    "/var/data:/data",
]

# ── PORTS ────────────────────────────────────────────────────────────────
# Informational — displayed in the deployment summary.
# Use 'melisa tunnel <container> <port>' to expose these to your workstation.
[ports]
expose = ["8080", "5432"]

# ── LIFECYCLE HOOKS ──────────────────────────────────────────────────────
# Shell commands executed inside the container at specific points.
# Commands run sequentially; a failed command logs a warning but does not halt deployment.
[lifecycle]
on_create = [
    "mkdir -p /app/logs",
    "chown -R www-data:www-data /app",
    "pip3 install -r /app/requirements.txt",
]
on_start = []           # Reserved for future use (not yet executed by --up).
on_stop  = [
    "nginx -s stop",
]

# ── SERVICES ─────────────────────────────────────────────────────────────
# Named service definitions. Informational — displayed in the deployment summary.
# Use 'melisa --send <container> <command>' to start them.
[services]
web    = { command = "gunicorn -w 4 app:app", working_dir = "/app", enabled = true }
worker = { command = "python3 worker.py",     working_dir = "/app", enabled = false }

# ── HEALTH CHECK ─────────────────────────────────────────────────────────
# Runs after lifecycle hooks. Optional.
[health]
command  = "curl -sf http://localhost:8080/health"  # Shell command to test inside container
interval = 5     # Seconds between retries (default: 5)
retries  = 3     # Max attempts before giving up (default: 3)
timeout  = 10    # Seconds per attempt (default: 10)
```

---

## Container Naming

If `[container].name` is not specified, the container name is derived from the project name:
- Spaces replaced by hyphens
- Converted to lowercase

Examples:

| `[project].name` | Effective container name |
|-----------------|------------------------|
| `myapp` | `myapp` |
| `My Web App` | `my-web-app` |
| `Backend-API` | `backend-api` |

---

## Dependency Resolution

The Deployment Engine detects which package manager is installed inside the container by running `which <pm>` silently for each candidate in order: `apt-get`, `pacman`, `dnf`, `apk`, `zypper`. The first found is used for all system dependency operations.

Language-level managers (`pip`, `npm`, `cargo`, `gem`, `composer`) are always attempted if their respective lists are non-empty, regardless of which system package manager was detected.

---

## Environment Variable Injection

Each `[env]` key-value pair is injected into `/etc/environment` inside the container using an idempotent sed command:

```bash
sed -i '/^KEY=/d' /etc/environment && echo 'KEY=VALUE' >> /etc/environment
```

This ensures repeated `--up` calls don't accumulate duplicate entries. Variables take effect for new processes inside the container; existing processes are not affected until they restart.

---

## Usage Patterns

**Deploy and enter:**
```
melisa@host:~> melisa --up ./myapp/program.mel
melisa@host:~> melisa --use myapp
```

**Inspect before deploying:**
```
melisa@host:~> melisa --mel-info ./myapp/program.mel
```

**Stop gracefully:**
```
melisa@host:~> melisa --down ./myapp/program.mel
```

**From the client (workstation):**
```bash
# Forward the command to the server
melisa --up ./myapp/program.mel
melisa --mel-info ./myapp/program.mel
melisa --down ./myapp/program.mel
```