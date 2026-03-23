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

# Fungsi untuk mengunduh project secara penuh beserta riwayat git
exec_clone() {
    ensure_connected
    
    local project_name=""
    local force_clone=false

    # Parsing argumen agar flag --force terdeteksi
    while [[ $# -gt 0 ]]; do
        case $1 in
            --force)
                force_clone=true
                shift
                ;;
            *)
                if [ -z "$project_name" ]; then
                    project_name=$1
                fi
                shift
                ;;
        esac
    done

    if [ -z "$project_name" ]; then
        log_error "Usage: melisa clone <project_name> [--force]"
        exit 1
    fi

    if [ "$force_clone" = true ]; then
        log_info "Mengambil SEMUA file (Force Mode) dari server via Rsync..."
        # Mengarah ke home directory di server (~/)
        local remote_path="~/$project_name/" 
        rsync -avz --progress "$CONN:$remote_path" "./$project_name"
        
        if [ $? -eq 0 ]; then
            log_success "Force clone berhasil! Semua file fisik (termasuk .env & vendor) ditarik."
        else
            log_error "Gagal rsync. Pastikan path di server benar."
        fi
    else
        log_info "Mengunduh project via Git (Master Repo)..."
        git clone "ssh://$CONN/opt/melisa/projects/$project_name"
    fi
}

# Fungsi sinkronisasi (The Boss Mode)
# Fungsi sinkronisasi (The Boss Mode)
exec_sync() {
    ensure_connected
    
    if [ ! -d .git ]; then
        log_error "Folder ini bukan repositori Git. Gunakan 'melisa clone' dulu."
        exit 1
    fi

    local project_name=$(basename "$PWD")
    local current_branch=$(git branch --show-current)

    log_info "Menyingkronkan perubahan '$project_name' ke Master..."
    
    # 1. Simpan perubahan lokal (termasuk file baru)
    git add .
    git commit -m "Update via Melisa Client: $(date)" --allow-empty
    
    # 2. FORCE PUSH: Laptop kamu adalah sumber kebenaran.
    log_info "Mengirim data ke Master Repo (Force Push)..."
    git push -f origin "$current_branch"
    
    if [ $? -eq 0 ]; then
        log_info "Push berhasil! Memperbarui folder fisik di server..."
        
        # 3. Panggil update di server
        ssh "$CONN" "melisa --update $project_name --force"
        
        # --- FIX: INJECT KHUSUS UNTUK FILE .env ---
        log_info "Menyalin konfigurasi lokal (.env) ke server..."
        # Cari semua file .env di dalam project lokal, lalu transfer via rsync 
        # dengan struktur folder yang sama (-R / --relative)
        find . -type f -name ".env" | while read -r env_file; do
            rsync -azR "$env_file" "$CONN:~/$project_name/" > /dev/null 2>&1
        done
        # ------------------------------------------
        
        log_success "Sinkronisasi selesai! Server sekarang identik dengan laptop."
    else
        log_error "Gagal mengirim data. Cek koneksi atau izin folder /opt/melisa/projects."
    fi
}

exec_forward() {
    ensure_connected
    # Tambahkan -t agar animasi spinner dan warna muncul
    ssh -t "$CONN" "melisa $*" 
}