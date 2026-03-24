# File: db.sh
DB_PATH="$HOME/.config/melisa/registry"
mkdir -p "$(dirname "$DB_PATH")"
touch "$DB_PATH"

# Helper untuk menangani perbedaan sed antara Linux dan macOS
sed_wrapper() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "$@"
    else
        sed -i "$@"
    fi
}

db_update_project() {
    local name=$1
    local path=$2
    
    if [ -z "$name" ] || [ -z "$path" ]; then return 1; fi

    # Menghapus entri lama jika ada (menggunakan delimiter | agar aman)
    sed_wrapper "\|^$name|d" "$DB_PATH"
    
    # Menambahkan entri baru
    echo "$name|$path" >> "$DB_PATH"
}

db_get_path() {
    # Menambahkan 'head -n 1' untuk memastikan hanya satu path yang diambil
    grep "^$1|" "$DB_PATH" | head -n 1 | cut -d'|' -f2
}

db_identify_by_pwd() {
    # Menggunakan $ di akhir untuk match path yang presisi
    grep -F "|$PWD" "$DB_PATH" | grep "|$PWD$" | head -n 1 | cut -d'|' -f1
}