# Testing — Ikhtisar

MELISA memiliki dua suite pengujian yang terpisah karena arsitekturnya terbagi dua: server ditulis dalam Rust, client ditulis dalam Bash dengan test suite Python.

---

## Struktur Test

```
melisa-on-saferoom-project/
├── src/
│   ├── deployment/
│   │   └── tests.rs              ← Rust unit & integration tests (server-side)
│   └── tests/
│       └── mod.rs                ← Rust test module root
└── src/melisa_client/
    └── ut_/
        ├── test_melisa.py                 ← Python unit & integration tests (client-side)
        └── test_tunnel_and_crossregion.py ← Tunnel & state machine tests
```

---

## Dua Lapisan Pengujian

### Lapisan 1 — Rust Tests (`src/deployment/tests.rs`)

Menguji logika inti server:
- Parsing dan validasi file `.mel` (via `mel_parser.rs`)
- Pembangunan perintah instalasi dependency (via `dependency.rs`)
- Logika deployment Engine (via `deployer.rs`)
- Pipeline integrasi end-to-end: parse `.mel` → build command

Dijalankan dengan `cargo test`.

**Panduan lengkap:** [Menulis Rust Tests](./rust-tests.md)

---

### Lapisan 2 — Python Tests (`src/melisa_client/ut_/`)

Menguji logika client-side Bash tanpa memerlukan server atau koneksi SSH:
- Validasi argumen dan port
- Format dan parsing file state tunnel
- Logika state machine `tunnel-list`
- Logic inti dari `exec.sh` (sync, clone, auth)

Dijalankan dengan `python3`.

**Panduan lengkap:** [Menulis Python Tests](./python-tests.md)

---

## Menjalankan Semua Test

```bash
# Rust tests
cargo test

# Python client tests
cd src/melisa_client/ut_
python3 test_melisa.py
python3 test_tunnel_and_crossregion.py
```

**Panduan lengkap:** [Menjalankan Tests](./running-tests.md)

---

## Filosofi Pengujian

**Unit tests** menguji satu fungsi atau logika terisolasi — tanpa I/O nyata, tanpa container, tanpa SSH.

**Integration tests** menguji pipeline multi-langkah dengan file sementara nyata (menggunakan `tempfile`/`NamedTempFile`) tapi tetap tanpa infrastruktur live.

**Tidak ada end-to-end tests** yang memerlukan server hidup di suite ini — itu adalah keputusan disengaja agar test bisa dijalankan oleh siapa saja di mesin mana saja tanpa setup infrastruktur.