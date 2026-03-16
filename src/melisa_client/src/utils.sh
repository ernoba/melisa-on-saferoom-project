# Definisi Warna untuk CLI Klien
BOLD="\e[1m"
RESET="\e[0m"
GREEN="\e[32m"
BLUE="\e[34m"
CYAN="\e[36m"
YELLOW="\e[33m"
RED="\e[31m"

log_info() { echo -e "${BOLD}${BLUE}[INFO]${RESET} $1"; }
log_success() { echo -e "${BOLD}${GREEN}[SUCCESS]${RESET} $1"; }
log_warning() { echo -e "${BOLD}${YELLOW}⚠️ [WARNING]${RESET} $1"; }
log_error() { echo -e "${BOLD}${RED}[ERROR]${RESET} $1"; }

# Pastikan SSH key lokal ada
ensure_ssh_key() {
    if [ ! -f ~/.ssh/id_rsa ]; then
        log_info "Membuat SSH Key lokal baru..."
        ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa -N "" -q
    fi
}