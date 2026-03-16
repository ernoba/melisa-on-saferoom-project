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

exec_forward() {
    ensure_connected
    ssh "$CONN" "melisa $*"
}