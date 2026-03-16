ensure_connected() {
    CONN=$(get_active_conn)
    if [ -z "$CONN" ]; then
        log_error "Tidak ada server yang aktif! Tambahkan server dengan:"
        echo "  melisa auth add dev-server root@192.168.1.10"
        exit 1
    fi
}

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
    if [ "$ext" == "py" ]; then interpreter="python3"; fi
    if [ "$ext" == "js" ]; then interpreter="node"; fi
    
    log_info "Menyiapkan file '${BOLD}$filename${RESET}' di folder /tmp/ container..."
    
    # 1. Mengupload file tunggal ke folder /tmp di dalam container
    tar -czf - -C "$dir" "$filename" | ssh "$CONN" "melisa --upload $container /tmp" > /dev/null 2>&1
    
    log_success "Menjalankan mode interaktif (TTY)...\n"
    
    # 2. Menjalankan file secara interaktif (menggunakan flag -t pada SSH untuk Pseudo-Terminal)
    ssh -t "$CONN" "melisa --send $container $interpreter /tmp/$filename"
    
    # 3. Membersihkan (Menghapus) file sementara dari container agar tidak jadi sampah
    ssh "$CONN" "melisa --send $container rm /tmp/$filename" > /dev/null 2>&1
    
    echo -e "\n${BOLD}${GREEN}[SUCCESS]${RESET} Eksekusi selesai dan file sementara telah dihapus."
}

exec_forward() {
    ensure_connected
    ssh "$CONN" "melisa $*"
}