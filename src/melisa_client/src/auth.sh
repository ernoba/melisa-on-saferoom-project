#!/usr/bin/env bash
# ==============================================================================
# MELISA AUTHENTICATION & CONNECTION MANAGER
# Description: Handles remote server profiles, active session state, 
#              and SSH multiplexing for low-latency command execution.
# ==============================================================================

CONFIG_DIR="$HOME/.config/melisa"
PROFILE_FILE="$CONFIG_DIR/profiles.conf"
ACTIVE_FILE="$CONFIG_DIR/active"

# ------------------------------------------------------------------------------
# INITIALIZATION
# ------------------------------------------------------------------------------

# Initializes the local configuration directory and profile storage.
# Enforces strict permissions to prevent unauthorized access to server lists.
init_auth() {
    # Create the configuration directory if it doesn't exist
    mkdir -p "$CONFIG_DIR"
    
    # Enforce strict directory permissions (Only the owner can read/write/execute)
    chmod 700 "$CONFIG_DIR" 2>/dev/null
    
    # Create the profile file if it doesn't exist
    touch "$PROFILE_FILE"
    
    # Enforce strict file permissions (Only the owner can read/write)
    chmod 600 "$PROFILE_FILE" 2>/dev/null
}

# ------------------------------------------------------------------------------
# CORE GETTERS
# ------------------------------------------------------------------------------

# Retrieves the connection string (user@host) for the currently active profile.
# Returns 1 (failure) silently if no active profile is set.
get_active_conn() {
    # Fail silently if the active state file is missing
    if [ ! -f "$ACTIVE_FILE" ]; then return 1; fi
    
    local active=$(cat "$ACTIVE_FILE")
    
    # Extract the user@host string associated with the active profile name
    local conn=$(grep "^${active}=" "$PROFILE_FILE" | cut -d'=' -f2)
    
    # Fail silently if the profile exists in the active file but not in the config
    if [ -z "$conn" ]; then return 1; fi
    
    echo "$conn"
}

get_remote_user() {
    if [ ! -f "$ACTIVE_FILE" ]; then return 1; fi
    local active
    active=$(cat "$ACTIVE_FILE" 2>/dev/null)

    # Baca seluruh value dari profil (format: root@host|alice)
    local raw
    raw=$(grep "^${active}=" "$PROFILE_FILE" | cut -d'=' -f2)

    # Ekstrak bagian setelah "|" — ini adalah melisa username
    # Jika tidak ada "|", hasilnya kosong (profil lama yang belum diperbarui)
    echo "$raw" | cut -s -d'|' -f2
}

# ------------------------------------------------------------------------------
# PROFILE MANAGEMENT
# ------------------------------------------------------------------------------

# Registers a new remote server profile and configures SSH multiplexing.
auth_add() {
    local name=$1
    local user_host=$2

    if [ -z "$name" ] || [ -z "$user_host" ]; then
        log_error "Usage: melisa auth add <profile_name> <user@host>"
        exit 1
    fi

    ensure_ssh_key

    log_info "Deploying public SSH key to ${BOLD}${user_host}${RESET}..."
    log_info "Please prepare to enter the remote server password."
    ssh-copy-id "$user_host" || { log_error "Failed to establish a connection to the remote server."; exit 1; }

    # --- TAMBAHAN: Tanya melisa username ---
    # Alice mungkin SSH sebagai root, tapi punya username 'alice' di MELISA.
    # Tanpa ini, semua path berbasis ~/ akan salah (menunjuk ke /root/).
    local remote_melisa_user=""
    read -r -p "$(echo -e "${YELLOW}[SETUP]${RESET} Enter your MELISA username on this server (leave blank if same as SSH user): ")" remote_melisa_user

    # Jika dikosongkan, gunakan bagian user dari user@host sebagai fallback
    if [ -z "$remote_melisa_user" ]; then
        remote_melisa_user=$(echo "$user_host" | cut -d'@' -f1)
        log_info "Using SSH user '${remote_melisa_user}' as remote MELISA username."
    fi
    # --- AKHIR TAMBAHAN ---

    local host
    host=$(echo "$user_host" | cut -d'@' -f2)
    local user
    user=$(echo "$user_host" | cut -d'@' -f1)

    mkdir -p ~/.ssh/sockets
    chmod 700 ~/.ssh ~/.ssh/sockets 2>/dev/null
    touch ~/.ssh/config
    chmod 600 ~/.ssh/config 2>/dev/null

    if ! grep -q "Host $host" ~/.ssh/config 2>/dev/null; then
        cat <<EOF >> ~/.ssh/config

Host $host
    User $user
    ControlMaster auto
    ControlPath ~/.ssh/sockets/%r@%h:%p
    ControlPersist 10m
EOF
    fi

    # Simpan dengan format baru: name=root@ip|melisa_username
    if [ -f "$PROFILE_FILE" ]; then
        grep -v "^${name}=" "$PROFILE_FILE" > "${PROFILE_FILE}.tmp"
        mv "${PROFILE_FILE}.tmp" "$PROFILE_FILE"
    fi

    echo "${name}=${user_host}|${remote_melisa_user}" >> "$PROFILE_FILE"
    echo "$name" > "$ACTIVE_FILE"

    log_success "Server profile '${name}' registered. Remote MELISA user: ${BOLD}${remote_melisa_user}${RESET}"
}

# Safely removes an existing server profile from the local configuration.
auth_remove() {
    local name=$1
    
    if [ -z "$name" ]; then
        log_error "Usage: melisa auth remove <profile_name>"
        return 1
    fi

    # Verify the target profile actually exists in the configuration
    if ! grep -q "^${name}=" "$PROFILE_FILE"; then
        log_error "Server profile '${name}' was not found in the registry."
        return 1
    fi

    # Interactive protection prompt
    read -p "$(echo -e "${YELLOW}Are you sure you want to permanently remove the profile '${name}'? (y/N): ${RESET}")" confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        log_info "Profile deletion aborted by user."
        return 0
    fi

    # Remove the specific profile using a POSIX-compliant temporary file swap
    grep -v "^${name}=" "$PROFILE_FILE" > "${PROFILE_FILE}.tmp"
    mv "${PROFILE_FILE}.tmp" "$PROFILE_FILE"

    # State Resolution: If the deleted profile was currently active, clear the active state
    local active=$(cat "$ACTIVE_FILE" 2>/dev/null)
    if [ "$name" == "$active" ]; then
        rm -f "$ACTIVE_FILE"
        log_info "The active profile was deleted. Please use 'melisa auth switch' to select a new server."
    fi

    log_success "Server profile '${name}' has been successfully purged from the registry."
}

# Switches the active connection context to a different registered server.
auth_switch() {
    local name=$1
    
    if [ -z "$name" ]; then
        log_error "Usage: melisa auth switch <profile_name>"
        return 1
    fi

    if grep -q "^${name}=" "$PROFILE_FILE"; then
        echo "$name" > "$ACTIVE_FILE"
        log_success "Successfully switched active connection to server: ${BOLD}${name}${RESET}"
    else
        log_error "Server profile '${name}' not found! Execute 'melisa auth list' to view available profiles."
    fi
}

# Displays an enumerated list of all registered remote servers.
auth_list() {
    local active=$(cat "$ACTIVE_FILE" 2>/dev/null)
    
    echo -e "\n${BOLD}${CYAN}=== MELISA REMOTE SERVER REGISTRY ===${RESET}"
    
    if [ ! -s "$PROFILE_FILE" ]; then
        echo "No servers are currently registered. Add one using 'melisa auth add <name> <user@host>'."
        return
    fi
    
    # Iterate through the configuration file and display formatted output
    while IFS='=' read -r name conn; do
        # Ignore empty lines
        if [ -z "$name" ]; then continue; fi 
        
        if [ "$name" == "$active" ]; then
            echo -e "  ${GREEN}* ${name}${RESET} \t(${conn}) ${YELLOW}<- [ACTIVE]${RESET}"
        else
            echo -e "    ${name} \t(${conn})"
        fi
    done < "$PROFILE_FILE"
    echo ""
}