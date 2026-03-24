# --- UI Helpers (Minimalist & Clean) ---
# Definisi warna agar variabel ${GREEN} dkk tidak kosong
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

source "$MELISA_LIB/db.sh"

ensure_connected() {
    CONN=$(get_active_conn)
    if [ -z "$CONN" ]; then
        log_error "Tidak ada server yang aktif!"
        echo "  melisa auth add dev-server root@192.168.1.10"
        exit 1
    fi
}

# --- Core Functions ---
exec_run() {
    ensure_connected
    local container=$1
    local file=$2
    
    if [ -z "$container" ] || [ -z "$file" ] || [ ! -f "$file" ]; then
        log_error "Usage: melisa run <container> <file>"
        exit 1
    fi
    
    local ext="${file##*.}"
    local interpreter="bash"
    if [ "$ext" == "py" ]; then interpreter="python3"; fi
    if [ "$ext" == "js" ]; then interpreter="node"; fi
    
    log_info "Menjalankan '${BOLD}$file${RESET}' di '$container' via server '$CONN'..."
    cat "$file" | ssh "$CONN" "melisa --send $container $interpreter -"
}

exec_upload() {
    ensure_connected
    local container=$1
    local dir=$2
    local dest=$3
    
    if [ -z "$dest" ]; then
        log_error "Usage: melisa upload <container> <local_dir> <remote_dest>"
        exit 1
    fi
    
    log_info "Mengupload '$dir' ke '$container:$dest' via server '$CONN'..."
    tar -czf - -C "$dir" . | ssh "$CONN" "melisa --upload $container $dest"
}


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
    
    log_info "Menyiapkan file '${BOLD}$filename${RESET}' di container..."
    
    # Gunakan petik ganda untuk path agar aman dari spasi
    if tar -czf - -C "$dir" "$filename" | ssh "$CONN" "melisa --upload $container /tmp" > /dev/null 2>&1; then
        log_success "Mode interaktif (TTY) dimulai..."
        ssh -t "$CONN" "melisa --send $container $interpreter /tmp/$filename"
        ssh "$CONN" "melisa --send $container rm /tmp/$filename" > /dev/null 2>&1
        log_success "Eksekusi selesai."
    else
        log_error "Gagal upload file ke container."
    fi
}



# Fungsi untuk melihat isi folder setelah aksi
inspect_result() {
    local target=$1
    echo -e "\n\e[2m[Current State: $target]\e[0m"
    
    local files=$(find "$target" -type f | wc -l)
    local dirs=$(find "$target" -type d | wc -l)
    local size=$(du -sh "$target" | cut -f1)

    log_stat "Files" "$files"
    log_stat "Dirs"  "$dirs"
    log_stat "Size"  "$size"
    
    echo -e "\n\e[1;30mStruktur Project (Depth 2):\e[0m"
    # Menggunakan find untuk simulasi tree sederhana 2 level
    # Menghapus '.' agar tampilan lebih bersih
    find "$target" -maxdepth 2 -not -path '*/.*' | sed "s|$target||" | sed 's|^/||' | grep -v "^$" | head -n 15 | sed 's/^/  /'
    
    [ "$files" -gt 15 ] && echo "  ..."
    echo ""
}
# --- Core Functions ---

exec_clone() {
    ensure_connected
    
    local project_name=""
    local force_clone=false

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

    log_header "Cloning $project_name"

    if [ "$force_clone" = true ]; then
        log_info "Mode: Force (Rsync direct)"
        local remote_path="~/$project_name/" 
        if rsync -avz --progress "$CONN:$remote_path" "./$project_name"; then
            local full_path="$(realpath "./$project_name")"
            db_update_project "$project_name" "$full_path"
            log_success "Sync complete."
            inspect_result "./$project_name"
        else
            log_error "Rsync failed. Check server path."
        fi
    else
        log_info "Mode: Git (Default)"
        if git clone "ssh://$CONN/opt/melisa/projects/$project_name"; then
            local full_path="$(realpath "./$project_name")"
            db_update_project "$project_name" "$full_path"
            log_success "Repo cloned."
            inspect_result "./$project_name"
        else
            log_error "Git clone failed."
        fi
    fi
}

exec_sync() {
    ensure_connected
    
    if [ ! -d .git ]; then
        log_error "Not a git repo. Clone it first."
        exit 1
    fi

    local project_name=$(basename "$PWD")
    local branch=$(git branch --show-current)

    log_header "Syncing $project_name [$branch]"
    
    # Preview apa yang mau dipush
    git status --short
    
    git add .
    git commit -m "melisa-sync: $(date +'%Y-%m-%d %H:%M')" --allow-empty > /dev/null
    
    log_info "Pushing to master..."
    if git push -f origin "$branch" 2>&1 | sed 's/^/  /'; then
        log_info "Updating server-side workdir..."
        ssh "$CONN" "melisa --update $project_name --force"
        
        # Sync .env files
        log_info "Injecting configs (.env)..."
        find . -type f -name ".env" | while read -r env_file; do
            rsync -azR "$env_file" "$CONN:~/$project_name/"
        done
        
        log_success "Server is now up-to-date."
        inspect_result "."
    else
        log_error "Push failed."
    fi
}

exec_get() {
    ensure_connected
    
    local project_name=$1
    local force_get=false
    
    # Perbaikan parsing: cek jika argumen kedua adalah --force
    [[ "$2" == "--force" ]] && force_get=true

    if [ -z "$project_name" ]; then
        project_name=$(db_identify_by_pwd)
        [ -z "$project_name" ] && { log_error "Project tidak teridentifikasi. Masukkan nama project."; exit 1; }
    fi

    # Mencari path lokal dari database atau folder saat ini
    local local_path=$(db_get_path "$project_name")
    [ -z "$local_path" ] && local_path="$PWD/$project_name"

    log_header "Pulling $project_name data"

    local remote_path="~/$project_name/"
    
    # 1. Tentukan flag rsync
    local opts="-avz --progress --exclude='.git/'"
    
    if [ "$force_get" = false ]; then
        log_info "Mode: Safe (Hanya ambil file baru, abaikan yang sudah ada)"
        # GANTI -u DENGAN --ignore-existing
        opts="$opts --ignore-existing"
    else
        log_info "Mode: Force (Sinkronisasi penuh, file lokal akan ditimpa)"
    fi

    # 2. Eksekusi rsync
    # Pastikan local_path ada agar tidak berantakan
    mkdir -p "$local_path"

    if rsync $opts "$CONN:$remote_path" "$local_path/"; then
        log_success "Data berhasil ditarik."
        inspect_result "$local_path"
    else
        log_error "Terjadi kesalahan pada Rsync."
    fi
}

exec_forward() {
    ensure_connected
    log_header "Remote command: melisa $*"
    ssh -t "$CONN" "melisa $*" 
}