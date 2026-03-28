# Menjalankan Tests

---

## Prasyarat

### Untuk Rust Tests

```bash
# Rust toolchain (minimal 1.70+)
rustup --version

# Dependensi test (sudah ada di Cargo.toml)
# - tokio (async runtime)
# - tempfile (temporary files)
```

Tidak ada prasyarat tambahan. Test Rust berjalan tanpa server, container, atau koneksi jaringan.

### Untuk Python Tests

```bash
# Python 3.8+
python3 --version

# Tidak ada dependensi eksternal — hanya stdlib
# (unittest, tempfile, shutil, subprocess, pathlib sudah built-in)
```

---

## Menjalankan Rust Tests

### Semua Tests

```bash
cargo test
```

Output saat semua test lolos:

```
running 35 tests
test tests::tests_mel_parser::test_valid_manifest_parses_correctly ... ok
test tests::tests_mel_parser::test_missing_required_field_returns_error ... ok
test tests::tests_dependency::test_build_update_cmd_apt ... ok
...
test tests::tests_integration::test_lifecycle_hooks_order_preserved ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Filter Tests

```bash
# Jalankan hanya modul parser
cargo test tests_mel_parser

# Jalankan hanya modul dependency
cargo test tests_dependency

# Jalankan satu test spesifik (partial match)
cargo test test_effective_name

# Tampilkan output println! dari dalam test
cargo test -- --nocapture

# Jalankan test secara serial (untuk debug race conditions)
cargo test -- --test-threads=1
```

### Verbose Output

```bash
cargo test -- --nocapture 2>&1 | head -50
```

---

## Menjalankan Python Tests

### Semua Tests di Satu File

```bash
cd src/melisa_client/ut_

python3 test_melisa.py
python3 test_tunnel_and_crossregion.py
```

Output saat semua test lolos:

```
....................
----------------------------------------------------------------------
Ran 20 tests in 0.045s

OK
```

### Verbose (Nama Setiap Test)

```bash
python3 -m unittest test_melisa -v
```

```
test_auth_add_creates_profile (test_melisa.TestAuthProfile) ... ok
test_auth_use_sets_active_profile (test_melisa.TestAuthProfile) ... ok
test_db_update_project (test_melisa.TestDbRegistry) ... ok
...
----------------------------------------------------------------------
Ran 20 tests in 0.047s

OK
```

### Filter Satu Class atau Method

```bash
# Jalankan satu TestCase class
python3 -m unittest test_tunnel_and_crossregion.TestTunnelPortValidation -v

# Jalankan satu test method
python3 -m unittest test_tunnel_and_crossregion.TestTunnelPortValidation.test_valid_port_numbers -v

# Jalankan semua test dari kedua file sekaligus
python3 -m unittest discover -s . -p "test_*.py" -v
```

---

## Menjalankan Semua Tests Sekaligus

Script `src/run_tests.py` menjalankan Rust tests dan Python tests dalam satu perintah:

```bash
# Dari root repository
python3 src/run_tests.py
```

Atau jalankan manual dalam urutan yang sama dengan CI:

```bash
# 1. Build check
cargo build 2>&1

# 2. Rust tests
cargo test 2>&1

# 3. Python client tests
cd src/melisa_client/ut_
python3 -m unittest discover -s . -p "test_*.py" -v 2>&1
cd ../../..
```

---

## Memahami Output Kegagalan

### Kegagalan Rust

```
test tests::tests_mel_parser::test_valid_manifest_parses_correctly ... FAILED

failures:

---- tests::tests_mel_parser::test_valid_manifest_parses_correctly stdout ----
thread 'tests::tests_mel_parser::test_valid_manifest_parses_correctly' panicked at
'Manifest valid harus berhasil di-parse, bukan: Err(missing field `name`)',
src/deployment/tests.rs:45:9
```

Baris yang gagal: `src/deployment/tests.rs:45`. Pesan error di assertion menjelaskan apa yang seharusnya terjadi. Error asli dari parser ada di `: Err(...)`.

### Kegagalan Python

```
FAIL: test_meta_file_has_correct_fields (test_tunnel_and_crossregion.TestTunnelFileManagement)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "test_tunnel_and_crossregion.py", line 87, in test_meta_file_has_correct_fields
    self.assertEqual(data["container"], "myapp")
AssertionError: 'wrongapp' != 'myapp'
```

Traceback menunjukkan file, baris, dan nilai aktual vs yang diharapkan.

---

## Tests di CI

Saat membuka Pull Request, CI menjalankan urutan berikut:

```yaml
# Urutan CI (referensi, bukan file aktual)
1. cargo fmt --check         # formatting check
2. cargo clippy -- -D warnings  # linting
3. cargo build               # compile check
4. cargo test                # Rust tests
5. python3 -m unittest discover -s src/melisa_client/ut_ -p "test_*.py"  # Python tests
```

PR tidak dapat di-merge jika salah satu langkah gagal.

---

## Troubleshooting

**`error[E0432]: unresolved import` saat `cargo test`**

Pastikan fungsi yang ingin diuji sudah dideklarasikan `pub` di module yang bersangkutan. Test berada di `tests.rs` yang me-import dari `crate::deployment::*`.

**Python test: `ModuleNotFoundError`**

Test Python hanya menggunakan stdlib. Jika ada `ModuleNotFoundError`, pastikan kamu menjalankan `python3` (bukan `python`) dan versinya minimal 3.8.

**Python test: `skipTest("Bash modules tidak tersedia")`**

Test yang menggunakan `BashEnv` melakukan skip otomatis jika file `.sh` tidak ditemukan di `src/melisa_client/src/`. Ini bukan kegagalan — test hanya tidak relevan di environment tanpa client source. Jalankan dari dalam direktori repository lengkap untuk test penuh.

**`cargo test` berjalan lambat**

Tambahkan `-- --test-threads=4` untuk paralelisme eksplisit, atau jalankan hanya modul yang relevan dengan filter nama:

```bash
cargo test tests_mel_parser -- --test-threads=4
```