# Bash Client Internals

## Module Loading Architecture

The client entry point (`melisa`) uses Bash's `source` command to load all modules into a single process context. This means all functions are available globally without subshell overhead:

```bash
# Entry point startup sequence
set -o pipefail

export MELISA_LIB="$HOME/.local/share/melisa"

# Pre-flight: verify SSH is available
if ! command -v ssh >/dev/null 2>&1; then
    echo "[FATAL ERROR] ssh not found"
    exit 1
fi

# Module integrity check (anti-silent-fail)
for module in "utils.sh" "auth.sh" "exec.sh"; do
    if [ ! -f "$MELISA_LIB/$module" ]; then
        echo "[FATAL ERROR] Core module '$module' is missing"
        exit 1
    fi
done

# Load all modules into current process
source "$MELISA_LIB/utils.sh"
source "$MELISA_LIB/auth.sh"
source "$MELISA_LIB/exec.sh"
# db.sh is sourced by exec.sh (exec.sh depends on it)

# Initialize auth subsystem
init_auth
```

The **load order matters**:
1. `utils.sh` — defines logging functions used by all other modules
2. `auth.sh` — defines `get_active_conn` used by `exec.sh`
3. `exec.sh` — sources `db.sh` internally, depends on `get_active_conn` from `auth.sh`

---

## The Project Registry: `db.sh`

The local project registry is a pipe-delimited flat file:

```
~/.config/melisa/registry:
myapp|/home/user/projects/myapp
backend|/home/user/work/company/backend
scripts|/home/user/tools/scripts
```

### `db_update_project` — Atomic POSIX Update

Uses a grep-filter-overwrite pattern (POSIX-compliant, works without GNU `sed -i`):

```bash
db_update_project() {
    local name=$1
    local path=$(realpath "$path" 2>/dev/null || echo "$path")

    # Remove existing entry atomically
    if [ -f "$DB_PATH" ]; then
        grep -v "^${name}|" "$DB_PATH" > "${DB_PATH}.tmp"
        mv "${DB_PATH}.tmp" "$DB_PATH"
    fi

    # Append updated entry
    echo "${name}|${path}" >> "$DB_PATH"
}
```

### `db_identify_by_pwd` — Longest Prefix Match

Identifies the current project by finding the **most specific** matching parent directory:

```bash
db_identify_by_pwd() {
    local current_dir=$(realpath "$PWD")
    local best_match_name=""
    local longest_path=0

    while IFS='|' read -r name path; do
        # Exact match or path is a parent of current dir
        if [[ "$current_dir" == "$path" ]] || \
           [[ "$current_dir" == "${path}/"* ]]; then
            # Prefer the deepest (longest) match
            if [ ${#path} -gt $longest_path ]; then
                longest_path=${#path}
                best_match_name="$name"
            fi
        fi
    done < "$DB_PATH"

    echo "$best_match_name"
}
```

The boundary check (`"${path}/"*` not just `"${path}"*`) prevents `/projects/app` from matching `/projects/apple`.

---

## SSH Multiplexing Deep Dive

When `auth add` configures multiplexing, it creates `~/.ssh/sockets/` and writes to `~/.ssh/config`:

```
Host 192.168.1.100
  User root
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h:%p
  ControlPersist 10m
```

The `ControlPath` pattern `%r@%h:%p` expands at runtime to the actual connection parameters — for example, `root@192.168.1.100:22` — and resolves to a Unix domain socket file at `~/.ssh/sockets/root@192.168.1.100:22`.

> **Note:** Previous versions used `ControlPath ~/.ssh/melisa_mux_%h_%p_%r`. If you have old socket files under that pattern, you can remove them with `rm -f ~/.ssh/melisa_mux_*`. Current installations use `~/.ssh/sockets/`.

**How it works:**

1. First `ssh` command to `192.168.1.100` creates a master connection and a Unix domain socket at the `ControlPath`
2. All subsequent SSH calls to the same host reuse the existing socket — no TCP handshake, no SSH key exchange, no authentication
3. The socket persists for 10 minutes after the last use (`ControlPersist 10m`)

**Impact on MELISA performance:** A command like `melisa --list` (which SSHes to run `melisa --list` on the server) takes ~50ms with multiplexing vs ~500–2000ms for a fresh connection each time.

---

## Profile Storage Format: `auth.sh`

The client stores server profiles in `~/.config/melisa/profiles.conf` using a pipe-extended key-value format:

```
name=user@host|melisa_username
```

For example:
```
myserver=root@192.168.1.100|alice
```

The two-field design separates **SSH transport** (`root@192.168.1.100`) from **MELISA application identity** (`alice`). This matters when you SSH as `root` but your projects and workspace on the server are under `/home/alice/`.

Three getter functions handle the resolution:

| Function | Returns |
|----------|---------|
| `get_active_conn` | SSH connection only (`root@192.168.1.100`) — strips `\|melisa_user` |
| `get_remote_user` | MELISA username only (`alice`) — part after `\|` |
| `get_active_melisa_user` | MELISA username with fallback to SSH user |

---

## `exec_sync` Pipeline Breakdown

The `sync` command is the most complex client operation:

```bash
exec_sync() {
    ensure_connected  # Abort if no active server

    # 1. Find project context from path registry
    local project_name=$(db_identify_by_pwd)
    # Abort if current dir isn't a registered MELISA project

    # 2. Navigate to absolute project root
    local project_root=$(db_get_path "$project_name")
    cd "$project_root"

    # 3. Determine current branch
    local branch=$(git branch --show-current 2>/dev/null || echo "master")

    # 4. Stage all changes
    git add .

    # 5. Auto-commit (--allow-empty handles "nothing to commit")
    git commit -m "melisa-sync: $(date +'%Y-%m-%d %H:%M')" --allow-empty

    # 6. Force push (local state wins)
    git push -f origin "$branch"

    # 7. SSH: trigger server-side hard reset
    ssh "$CONN" "melisa --update $project_name --force"

    # 8. Sync .env files via rsync with relative paths (-R flag)
    local env_files=$(find . -maxdepth 2 -type f -name ".env")
    if [ -n "$env_files" ]; then
        echo "$env_files" | xargs -I {} rsync -azR "{}" "$CONN:~/$project_name/"
    fi
}
```

**Why `--allow-empty`?** Without it, `git commit` fails if there are no staged changes, breaking the sync pipeline. With it, even a "nothing changed" sync creates a commit and pushes successfully, ensuring the server always gets the latest `melisa --update` trigger.

**The `-R` flag in rsync:** The `-R` (relative) flag preserves the path structure relative to the rsync source. So `./config/.env` syncs to `~/myapp/config/.env` on the server, not `~/myapp/.env`.

---

## `exec_clone` — Anti-Nesting Protocol

```bash
local target_dir="./$project_name"

# If we're ALREADY inside a directory named after the project,
# clone in-place to prevent myapp/myapp nesting
if [ "$(basename "$PWD")" == "$project_name" ]; then
    target_dir="."
    log_info "Context Detected: Currently inside target directory."
fi
```

This handles the common workflow:
```bash
mkdir myapp && cd myapp
melisa clone myapp   # Without anti-nesting: creates myapp/myapp/
                     # With anti-nesting: clones into current dir
```

---

## Logging System

All logging functions route stderr-bound messages through `>&2` to avoid corrupting pipelines:

```bash
log_info()    { echo -e "${BOLD}${BLUE}[INFO]${RESET} $1"; }        # stdout
log_success() { echo -e "${BOLD}${GREEN}[SUCCESS]${RESET} $1"; }    # stdout
log_warning() { echo -e "${BOLD}${YELLOW}⚠️ [WARNING]${RESET} $1" >&2; }  # stderr
log_error()   { echo -e "${BOLD}${RED}[ERROR]${RESET} $1" >&2; }   # stderr
```

This means `melisa clone myapp 2>/dev/null` will suppress errors but still show progress, and piping `melisa get myapp | tee log.txt` captures progress but not errors.

---

## `exec_upload` — Streaming Tar

```bash
exec_upload() {
    local container=$1
    local dir=$2
    local dest=$3

    # Compress the local directory and stream directly over SSH
    tar -czf - -C "$dir" . | ssh "$CONN" "melisa --upload $container $dest"
}
```

The `-C "$dir" .` changes to the source directory before archiving, so the archive contains the directory's **contents** rather than the directory itself. No temporary files are created anywhere — the stream flows: local disk → gzip → SSH → server → lxc-attach → container.