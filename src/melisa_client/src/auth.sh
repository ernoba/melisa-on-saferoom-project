CONFIG_DIR="$HOME/.config/melisa"
PROFILE_FILE="$CONFIG_DIR/profiles.conf"
ACTIVE_FILE="$CONFIG_DIR/active"

init_auth() {
    mkdir -p "$CONFIG_DIR"
    touch "$PROFILE_FILE"
}

get_active_conn() {
    if [ ! -f "$ACTIVE_FILE" ]; then return 1; fi
    local active=$(cat "$ACTIVE_FILE")
    # Mengambil format user@host dari profile
    local conn=$(grep "^${active}=" "$PROFILE_FILE" | cut -d'=' -f2)
    if [ -z "$conn" ]; then return 1; fi
    echo "$conn"
}

auth_add() {
    local name=$1
    local user_host=$2 # format: user@192.168.1.10
    
    if [ -z "$name" ] || [ -z "$user_host" ]; then
        log_error "Usage: melisa auth add <profile_name> <user@host>"
        exit 1
    fi

    ensure_ssh_key
    log_info "Menyalin kunci SSH ke ${user_host} (siapkan password Anda)..."
    ssh-copy-id "$user_host" || { log_error "Gagal menghubungkan ke server."; exit 1; }

    # Setup Multiplexing Otomatis
    local host=$(echo "$user_host" | cut -d'@' -f2)
    local user=$(echo "$user_host" | cut -d'@' -f1)
    
    mkdir -p ~/.ssh/sockets
    if ! grep -q "Host $host" ~/.ssh/config 2>/dev/null; then
        cat <<EOF >> ~/.ssh/config
Host $host
    User $user
    ControlMaster auto
    ControlPath ~/.ssh/sockets/%r@%h:%p
    ControlPersist 10m
EOF
    fi

    # Simpan ke config
    sed -i "/^${name}=/d" "$PROFILE_FILE" 2>/dev/null # Hapus jika nama sudah ada
    echo "${name}=${user_host}" >> "$PROFILE_FILE"
    echo "$name" > "$ACTIVE_FILE"
    
    log_success "Server '$name' ($user_host) berhasil ditambahkan dan dijadikan AKTIF!"
}

auth_switch() {
    local name=$1
    if grep -q "^${name}=" "$PROFILE_FILE"; then
        echo "$name" > "$ACTIVE_FILE"
        log_success "Berhasil beralih ke server: ${BOLD}$name${RESET}"
    else
        log_error "Server '$name' tidak ditemukan! Gunakan 'melisa auth list' untuk melihat."
    fi
}

auth_list() {
    local active=$(cat "$ACTIVE_FILE" 2>/dev/null)
    echo -e "\n${BOLD}${CYAN}=== DAFTAR SERVER MELISA ===${RESET}"
    if [ ! -s "$PROFILE_FILE" ]; then
        echo "Belum ada server. Tambahkan dengan 'melisa auth add'"
        return
    fi
    
    while IFS='=' read -r name conn; do
        if [ "$name" == "$active" ]; then
            echo -e "  ${GREEN}* $name${RESET} \t($conn) ${YELLOW}<- [AKTIF]${RESET}"
        else
            echo -e "    $name \t($conn)"
        fi
    done < "$PROFILE_FILE"
    echo ""
}