#!/bin/bash

# ==========================================
# MELISA MODULAR CLIENT INSTALLER
# ==========================================
BOLD="\e[1m"
RESET="\e[0m"
GREEN="\e[32m"
CYAN="\e[36m"

echo -e "${BOLD}${CYAN}Memasang Melisa Client...${RESET}"

# Buat direktori sistem lokal
mkdir -p ~/.local/bin
mkdir -p ~/.local/share/melisa
mkdir -p ~/.config/melisa

# Salin source file
cp src/melisa ~/.local/bin/melisa
cp src/*.sh ~/.local/share/melisa/

# Berikan izin eksekusi
chmod +x ~/.local/bin/melisa

# Daftarkan ke PATH (jika belum)
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
    export PATH="$HOME/.local/bin:$PATH"
fi

echo -e "${BOLD}${GREEN}[SUCCESS] Melisa berhasil dipasang!${RESET}"
echo -e "Silakan ketik ${BOLD}melisa${RESET} di terminal Anda."