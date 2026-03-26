#!/usr/bin/env bash
# ==============================================================================
# MELISA EXECUTION ENGINE
# Description: Handles remote code execution, project cloning, synchronization,
#              and artifact transfers via secure SSH pipelines.
# ==============================================================================

# --- UI Helpers (Minimalist & Clean) ---
# Define color variables explicitly to prevent empty variable evaluation errors
export BOLD='\e[1m'
export GREEN='\e[32m'
export RED='\e[31m'
export YELLOW='\e[33m'
export BLUE='\e[34m'
export RESET='\e[0m'

log_header()  { echo -e "\n${BLUE}::${RESET} ${BOLD}$1${RESET}"; }
log_stat()    { echo -e " ${GREEN}=>${RESET} $1: ${BOLD}$2${RESET}"; }
log_info()    { echo -e " ${BLUE}[INFO]${RESET} $1"; }
log_success() { echo -e " ${GREEN}[SUCCESS]${RESET} $1"; }
log_error()   { echo -ne " ${RED}[ERROR]${RESET} $1\n" >&2; }

# Source the local database module for path and project state resolution
source "$MELISA_LIB/db.sh"

# Validates that an active server connection is configured before proceeding
ensure_connected() {
    CONN=$(get_active_conn)
    if [ -z "$CONN" ]; then
        log_error "No active server connection found!"
        echo -e "  ${YELLOW}Tip:${RESET} Execute 'melisa auth add <name> <user@ip>' to register a server."
        exit 1
    fi
}

# ------------------------------------------------------------------------------
# REMOTE OPERATIONS (CONTAINER INTERACTION)
# ------------------------------------------------------------------------------

# Pipes a local script directly into a remote container's interpreter via SSH.
# Leaves zero footprint on the host machine.
exec_run() {
    ensure_connected
    local container=$1
    local file=$2
    
    if [ -z "$container" ] || [ -z "$file" ] || [ ! -f "$file" ]; then
        log_error "Usage: melisa run <container> <file>"
        exit 1
    fi
    
    # Dynamic interpreter resolution based on file extension
    local ext="${file##*.}"
    local interpreter="bash"
    if [ "$ext" == "py" ]; then interpreter="python3"; fi
    if [ "$ext" == "js" ]; then interpreter="node"; fi
    
    log_info "Executing '${BOLD}${file}${RESET}' inside '${container}' via server '${CONN}'..."
    # Stream the file content directly into the remote interpreter's STDIN
    cat "$file" | ssh "$CONN" "melisa --send $container $interpreter -"
}

# Compresses a local directory into a stream and extracts it directly inside the remote container.
exec_upload() {
    ensure_connected
    local container=$1
    local dir=$2
    local dest=$3
    
    if [ -z "$dest" ]; then
        log_error "Usage: melisa upload <container> <local_dir> <remote_dest>"
        exit 1
    fi
    
    log_info "Transferring '${dir}' to '${container}:${dest}' via server '${CONN}'..."
    # Tar stream execution: Compress locally, pipe over SSH, and extract remotely via MELISA
    tar -czf - -C "$dir" . | ssh "$CONN" "melisa --upload $container $dest"
}

# Uploads a script, executes it interactively (TTY), and cleans up afterward.
exec_run_tty() {
    ensure_connected
    local container=$1
    local file=$2
    
    if [ -z "$container" ] || [ -z "$file" ] || [ ! -f "$file" ]; then
        log_error "Usage: melisa run-tty <container> <file>"
        exit 1
    fi
    
    local filename=$(basename "$file")
    local dir=$(dirname "$file")
    local ext="${file##*.}"
    local interpreter="bash"
    [[ "$ext" == "py" ]] && interpreter="python3"
    [[ "$ext" == "js" ]] && interpreter="node"
    
    log_info "Provisioning artifact '${BOLD}${filename}${RESET}' in remote container..."
    
    # Securely upload the specific file to the container's /tmp directory
    if tar -czf - -C "$dir" "$filename" | ssh "$CONN" "melisa --upload $container /tmp" > /dev/null 2>&1; then
        log_success "Interactive session (TTY) initialized..."
        
        # Execute interactively (-t forces pseudo-tty allocation)
        ssh -t "$CONN" "melisa --send $container $interpreter /tmp/$filename"
        
        # Mandatory Cleanup Protocol
        ssh "$CONN" "melisa --send $container rm -f /tmp/$filename" > /dev/null 2>&1
        log_success "Execution cycle completed and artifacts purged."
    else
        log_error "Failed to transfer the artifact to the remote container."
    fi
}

# ------------------------------------------------------------------------------
# PROJECT ORCHESTRATION & SYNCHRONIZATION
# ------------------------------------------------------------------------------

# Visualizes the state of a directory after a synchronization event.
inspect_result() {
    local target=$1
    echo -e "\n\e[2m[Workspace State: $target]\e[0m"
    
    # Safely count entities, ignoring permission denied errors on restricted system files
    local files=$(find "$target" -type f 2>/dev/null | wc -l)
    local dirs=$(find "$target" -type d 2>/dev/null | wc -l)
    local size=$(du -sh "$target" 2>/dev/null | cut -f1)

    log_stat "Files" "$files"
    log_stat "Dirs"  "$dirs"
    log_stat "Size"  "$size"
    
    echo -e "\n\e[1;30mProject Topology (Depth 2):\e[0m"
    # Generate a clean, pseudo-tree visualization of the top two directory levels
    find "$target" -maxdepth 2 -not -path '*/.*' 2>/dev/null | sed "s|$target||" | sed 's|^/||' | grep -v "^$" | head -n 15 | sed 's/^/  /'
    
    [ "$files" -gt 15 ] && echo "  ..."
    echo ""
}

# Retrieves a project workspace from the master server via Git or Rsync.
exec_clone() {
    ensure_connected
    
    local project_name=""
    local force_clone=false

    # Robust argument parsing
    while [[ $# -gt 0 ]]; do
        case $1 in
            --force) force_clone=true; shift ;;
            *) [ -z "$project_name" ] && project_name=$1; shift ;;
        esac
    done

    if [ -z "$project_name" ]; then
        log_error "Usage: melisa clone <name> [--force]"
        exit 1
    fi

    log_header "Provisioning Workspace: $project_name"

    # --- ANTI-NESTING PROTOCOL ---
    # Prevents creating a folder inside a folder with the same name.
    local target_dir="./$project_name"
    if [ "$(basename "$PWD")" == "$project_name" ]; then
        target_dir="."
        log_info "Context Detected: Currently inside target directory. Syncing in place."
    fi

    if [ "$force_clone" = true ]; then
        log_info "Protocol: Force Overwrite (Direct Rsync)"
        local remote_path="~/$project_name/" 
        
        # Ensure the target directory exists if we aren't cloning in-place
        [ "$target_dir" != "." ] && mkdir -p "$target_dir"

        # Trailing slashes are CRITICAL for Rsync to copy contents rather than the directory itself
        if rsync -avz --progress "$CONN:$remote_path" "$target_dir/"; then
            local full_path="$(realpath "$target_dir")"
            db_update_project "$project_name" "$full_path"
            log_success "Synchronization complete at $full_path"
            inspect_result "$target_dir"
        else
            log_error "Rsync protocol failed. Verify server path and network connection."
        fi
    else
        log_info "Protocol: Version Control (Git Default)"
        
        # Git aborts if cloning into a non-empty directory. We trap this gracefully.
        if [ "$target_dir" == "." ] && [ "$(ls -A . 2>/dev/null)" ]; then
            log_error "Directory is not empty. Use '--force' for Rsync overwrite or navigate to an empty directory."
            exit 1
        fi

        if git clone "ssh://$CONN/opt/melisa/projects/$project_name" "$target_dir"; then
            local full_path="$(realpath "$target_dir")"
            db_update_project "$project_name" "$full_path"
            log_success "Repository successfully cloned to $full_path"
            inspect_result "$target_dir"
        else
            log_error "Git clone protocol failed."
        fi
    fi
}

# Pushes local changes to the remote repository and synchronizes untracked .env files.
exec_sync() {
    ensure_connected

    # --- PRE-FLIGHT: Pastikan git dan rsync tersedia ---
    # Bug lama: hanya ssh yang dicek di entry point, git/rsync tidak pernah dicek.
    for tool in git rsync; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            log_error "Required tool '${tool}' is not installed or not in PATH."
            log_error "Install it: apt install ${tool}  OR  dnf install ${tool}"
            exit 1
        fi
    done

    # 1. Identifikasi project dari registry path lokal
    local project_name
    project_name=$(db_identify_by_pwd)

    if [ -z "$project_name" ]; then
        log_error "The current directory is not registered as a MELISA project workspace."
        log_error "Run 'melisa clone <project>' first, or register manually:"
        log_error "  echo \"myapp|\$(realpath .)\" >> ~/.config/melisa/registry"
        exit 1
    fi

    # 2. Pindah ke root project
    local project_root
    project_root=$(db_get_path "$project_name")
    cd "$project_root" || { log_error "Failed to access workspace root: $project_root"; exit 1; }

    # 3. Validasi: pastikan ini adalah git repository
    # Bug lama: tidak ada validasi. Jika di-clone dengan --force (rsync, tanpa .git),
    # semua perintah git berikut akan gagal tanpa pesan error yang informatif.
    if [ ! -d ".git" ]; then
        log_error "The workspace at '${project_root}' is not a Git repository."
        log_error "This project was likely cloned with '--force' (Rsync mode)."
        log_error "To use 'sync', you need a proper Git clone:"
        log_error "  rm -rf ${project_root} && melisa clone ${project_name}"
        exit 1
    fi

    local branch
    branch=$(git branch --show-current 2>/dev/null || echo "master")
    log_header "Synchronizing $project_name [Branch: $branch]"

    # 4. Stage, commit, dan push ke bare repo
    git add .
    git commit -m "melisa-sync: $(date +'%Y-%m-%d %H:%M')" --allow-empty > /dev/null

    log_info "Transmitting delta to host server..."
    if ! git push -f origin "$branch" 2>&1 | sed 's/^/  /'; then
        log_error "Git push protocol failed. Verify network connectivity and remote configuration."
        log_error "Test manually: git remote -v"
        exit 1
    fi

    # --- CATATAN PENTING (Mengapa --update dihapus) ---
    # Sebelumnya ada: ssh "$CONN" "melisa --update $project_name --force"
    # Ini DIHAPUS karena:
    #   a) git push ke bare repo sudah memicu post-receive hook secara otomatis.
    #   b) Post-receive hook menjalankan: sudo melisa --update-all $project_name
    #      yang memperbarui workspace SEMUA user termasuk Alice.
    #   c) Pemanggilan SSH eksplisit di sini berjalan sebagai 'root' (bukan Alice),
    #      sehingga update_project mencari /home/root/$project_name yang tidak ada.
    #   d) Hasilnya: pemanggilan selalu gagal secara diam-diam → dead code.
    # Post-receive hook sudah cukup. Jika diperlukan verifikasi, gunakan:
    #   ssh "$CONN" "melisa --projects"  ← hanya untuk diagnostic

    # 5. Sync .env files — DIPERBAIKI
    # Bug lama: rsync ke "$CONN:~/$project_name/" yang = "/root/myapp/" (salah)
    # Fix: gunakan path absolut berdasarkan remote melisa username
    log_info "Synchronizing environment configurations (.env)..."

    local remote_user
    remote_user=$(get_remote_user)

    local env_files
    env_files=$(find . -maxdepth 2 -type f -name ".env")

    if [ -n "$env_files" ]; then
        if [ -n "$remote_user" ]; then
            # PATH BENAR: /home/alice/myapp/ bukan /root/myapp/
            local remote_env_path="/home/${remote_user}/${project_name}/"
            echo "$env_files" | xargs -I {} rsync -azR "{}" "$CONN:${remote_env_path}"
            log_success ".env files synced to ${remote_user}@server:${remote_env_path}"
        else
            # Fallback: remote_user belum dikonfigurasi (profil lama)
            # Beri peringatan agar user memperbarui profil mereka
            log_warning ".env sync SKIPPED: remote MELISA username not configured."
            log_warning "Update your profile: melisa auth remove ${CONN} && melisa auth add <name> ${CONN}"
            log_warning "Or manually: rsync .env root@server:/home/<your-username>/${project_name}/"
        fi
    else
        log_info "No .env files found within 2 directory levels. Skipping env sync."
    fi

    log_success "Synchronization complete. Server will propagate changes via post-receive hook."
}

# Pulls the latest physical data from the host workspace to the local machine via Rsync.
exec_get() {
    ensure_connected

    local project_name=""
    local force_get=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --force) force_get=true; shift ;;
            *) [ -z "$project_name" ] && project_name=$1; shift ;;
        esac
    done

    [ -z "$project_name" ] && project_name=$(db_identify_by_pwd)

    if [ -z "$project_name" ]; then
        log_error "Project context unknown. Usage: melisa get <n> [--force]"
        exit 1
    fi

    local local_path
    local_path=$(db_get_path "$project_name")

    if [ -z "$local_path" ]; then
        if [ "$(basename "$PWD")" == "$project_name" ]; then
            local_path="$(realpath .)"
        else
            local_path="$(realpath .)/$project_name"
        fi
    fi

    # --- PERBAIKAN PATH REMOTE ---
    # Bug lama: remote_path="~/$project_name/" → /root/$project_name/ (salah)
    # Fix: gunakan get_remote_user() untuk path yang benar
    local remote_user
    remote_user=$(get_remote_user)
    local remote_path

    if [ -n "$remote_user" ]; then
        # Path yang benar: workspace milik user Alice di server
        remote_path="/home/${remote_user}/${project_name}/"
    else
        # Fallback untuk profil lama — berikan peringatan
        log_warning "Remote MELISA username not configured. Falling back to ~/ (may point to /root/)."
        log_warning "Run 'melisa auth add' again to set your remote username."
        remote_path="~/${project_name}/"
    fi
    # --- AKHIR PERBAIKAN ---

    log_header "Retrieving Data for Workspace: $project_name"

    local opts="-avz --progress --exclude='.git/'"

    if [ "$force_get" = true ]; then
        log_info "Protocol: Force Overwrite (Data Replacement)"
    else
        log_info "Protocol: Safe Sync (Ignoring existing local files)"
        opts="$opts --ignore-existing"
    fi

    mkdir -p "$local_path"

    if rsync $opts "$CONN:$remote_path" "$local_path/"; then
        db_update_project "$project_name" "$local_path"
        log_success "Data retrieval completed at: $local_path"
        inspect_result "$local_path"
    else
        log_error "Rsync protocol failed."
        log_error "Verify workspace exists: ssh $CONN 'ls -la /home/$remote_user/'"
    fi
}


# Transparently forwards unrecognized commands directly to the MELISA host environment.
exec_forward() {
    ensure_connected
    log_header "Forwarding Payload: melisa $*"
    # -t enforces pseudo-tty allocation, allowing interactive remote commands
    ssh -t "$CONN" "melisa $*" 
}