# Menulis Python Tests (Client)

Test Python MELISA berada di `src/melisa_client/ut_/`. Suite ini menguji logika client-side Bash tanpa memerlukan server hidup, koneksi SSH, atau container aktif.

---

## Dua File Test

| File | Fokus |
|------|-------|
| `test_melisa.py` | Core client logic: auth, sync, clone, registry, exec |
| `test_tunnel_and_crossregion.py` | Tunnel port validation, file state management, dan list logic |

---

## Menjalankan Tests

```bash
cd src/melisa_client/ut_

# Jalankan semua test di satu file
python3 test_melisa.py
python3 test_tunnel_and_crossregion.py

# Verbose output (nama setiap test case ditampilkan)
python3 -m unittest test_melisa -v
python3 -m unittest test_tunnel_and_crossregion -v

# Jalankan satu TestCase class
python3 -m unittest test_melisa.TestAuthProfile -v

# Jalankan satu test method
python3 -m unittest test_melisa.TestAuthProfile.test_profile_format_valid -v
```

---

## Struktur File Test

Setiap file test mengikuti struktur standar Python `unittest`:

```python
import unittest
import tempfile
import shutil
import os
import subprocess
from pathlib import Path
from typing import Optional
import textwrap

# Konstanta untuk menemukan source client
CLIENT_SRC = None
_candidate = Path(__file__).parent.parent / "src"
if _candidate.is_dir():
    CLIENT_SRC = _candidate

class TestNamaKelasTest(unittest.TestCase):
    def setUp(self):
        """Dipanggil sebelum setiap test method."""
        self.tmp = tempfile.mkdtemp(prefix="melisa_test_")

    def tearDown(self):
        """Dipanggil setelah setiap test method — bersihkan state."""
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_sesuatu(self):
        # arrange
        # act
        # assert
        pass

if __name__ == "__main__":
    unittest.main()
```

---

## Kelas `BashEnv` — Menguji Bash Functions

`BashEnv` adalah test helper yang memungkinkan pengujian fungsi Bash dari Python. Ia membuat direktori temp yang mensimulasikan `~/.local/share/melisa/` dan `~/.config/melisa/`, lalu menjalankan snippet Bash dengan semua module di-source.

### Cara Menggunakan `BashEnv`

```python
class BashEnv:
    """
    Konteks test yang menyimulasikan environment instalasi client MELISA.
    Modul Bash (utils.sh, auth.sh, db.sh, exec.sh) di-source sebelum
    script test dijalankan.
    """
    def __init__(self):
        self.tmp_dir = tempfile.mkdtemp(prefix="melisa_bash_test_")
        self.lib_dir  = Path(self.tmp_dir) / ".local" / "share" / "melisa"
        self.conf_dir = Path(self.tmp_dir) / ".config" / "melisa"
        self.lib_dir.mkdir(parents=True)
        self.conf_dir.mkdir(parents=True)

        # Salin module Bash jika tersedia
        if CLIENT_SRC is not None:
            for sh in ["utils.sh", "auth.sh", "db.sh", "exec.sh"]:
                src = CLIENT_SRC / sh
                if src.exists():
                    shutil.copy(src, self.lib_dir / sh)

    def run(self, script: str, timeout: int = 10) -> tuple[int, str, str]:
        """
        Jalankan snippet Bash dalam konteks ini.
        Returns: (returncode, stdout, stderr)
        """
        env = os.environ.copy()
        env["HOME"]       = self.tmp_dir
        env["MELISA_LIB"] = str(self.lib_dir)
        env["MELISA_CONF"]= str(self.conf_dir)

        header = textwrap.dedent("""
            set -o pipefail
            MELISA_LIB="$MELISA_LIB"
            [ -f "$MELISA_LIB/utils.sh" ] && source "$MELISA_LIB/utils.sh" 2>/dev/null
            [ -f "$MELISA_LIB/auth.sh"  ] && source "$MELISA_LIB/auth.sh"  2>/dev/null
            [ -f "$MELISA_LIB/db.sh"    ] && source "$MELISA_LIB/db.sh"    2>/dev/null
            [ -f "$MELISA_LIB/exec.sh"  ] && source "$MELISA_LIB/exec.sh"  2>/dev/null
        """)
        full_script = header + "\n" + script
        try:
            result = subprocess.run(
                ["bash", "-c", full_script],
                capture_output=True, text=True,
                env=env, timeout=timeout
            )
            return result.returncode, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            return -1, "", f"TIMEOUT after {timeout}s"

    def cleanup(self):
        shutil.rmtree(self.tmp_dir, ignore_errors=True)
```

### Contoh Test dengan `BashEnv`

```python
class TestDbRegistry(unittest.TestCase):
    def setUp(self):
        self.env = BashEnv()

    def tearDown(self):
        self.env.cleanup()

    def test_db_update_and_get_project(self):
        # Pastikan modul tersedia dulu
        if not has_bash_modules():
            self.skipTest("Bash modules tidak tersedia")

        # Tulis entry ke registry dan baca kembali
        script = """
            DB_PATH="$MELISA_CONF/registry"
            db_update_project "myapp" "/home/user/projects/myapp"
            db_get_path "myapp"
        """
        rc, out, err = self.env.run(script)
        self.assertEqual(rc, 0)
        self.assertIn("/home/user/projects/myapp", out)

    def test_db_identify_by_pwd(self):
        if not has_bash_modules():
            self.skipTest("Bash modules tidak tersedia")

        script = """
            DB_PATH="$MELISA_CONF/registry"
            echo "myapp|/home/user/projects/myapp" > "$DB_PATH"
            cd /home/user/projects/myapp/src
            PWD=/home/user/projects/myapp/src
            db_identify_by_pwd
        """
        rc, out, _ = self.env.run(script)
        self.assertEqual(rc, 0)
        self.assertIn("myapp", out)
```

---

## Menguji Logic Murni (Tanpa Bash)

Sebagian besar test dapat dan sebaiknya ditulis sebagai Python murni — tidak memerlukan Bash sama sekali. Ini lebih cepat, lebih portable, dan tidak bergantung pada ketersediaan file `.sh`.

### Pola: Menguji Validasi Argumen

```python
class TestPortValidation(unittest.TestCase):
    """
    Mereplikasi logika validasi port dari exec.sh dalam Python
    untuk memastikan spesifikasi perilaku terdokumentasi.
    """

    def _validate_port(self, port_str: str) -> bool:
        """Mirror dari validasi port di exec.sh."""
        return (
            bool(port_str) and
            port_str.isdigit() and
            1 <= int(port_str) <= 65535
        )

    def test_valid_ports(self):
        for port in ["1", "80", "443", "3000", "8080", "65535"]:
            with self.subTest(port=port):
                self.assertTrue(self._validate_port(port))

    def test_reject_out_of_range(self):
        self.assertFalse(self._validate_port("0"),     "Port 0 tidak valid")
        self.assertFalse(self._validate_port("65536"),  "Port > 65535 tidak valid")
        self.assertFalse(self._validate_port("99999"),  "Port >> 65535 tidak valid")

    def test_reject_non_numeric(self):
        for bad in ["abc", "3.14", "3000abc", "", " ", "!@#"]:
            with self.subTest(bad=bad):
                self.assertFalse(self._validate_port(bad))

    def test_local_port_defaults_to_remote(self):
        remote = "8080"
        local  = ""   # tidak diisi user
        result = local if local else remote
        self.assertEqual(result, "8080")
```

### Pola: Menguji File State

Gunakan `tempfile.mkdtemp()` untuk membuat direktori sementara, lalu operasikan file state di dalamnya.

```python
class TestTunnelFileManagement(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_tunnel_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_meta(self, container: str, remote_port: int, local_port: int,
                    server: str, container_ip: str = "10.0.3.5") -> Path:
        """Helper: tulis file .meta untuk satu tunnel."""
        key  = f"{container}_{remote_port}"
        meta = self.tunnel_dir / f"{key}.meta"
        meta.write_text(textwrap.dedent(f"""\
            container={container}
            container_ip={container_ip}
            remote_port={remote_port}
            local_port={local_port}
            server={server}
            started=2026-03-20 16:30:00
        """))
        return meta

    def _write_pid(self, container: str, remote_port: int, pid: int) -> Path:
        """Helper: tulis file .pid untuk satu tunnel."""
        key      = f"{container}_{remote_port}"
        pid_file = self.tunnel_dir / f"{key}.pid"
        pid_file.write_text(str(pid) + "\n")
        return pid_file

    def _parse_meta(self, meta_file: Path) -> dict:
        """Helper: parse file .meta menjadi dict."""
        result = {}
        for line in meta_file.read_text().splitlines():
            if "=" in line:
                k, _, v = line.partition("=")
                result[k.strip()] = v.strip()
        return result

    # ─────────────────────────────────────────────────────────────────────────
    # Tests
    # ─────────────────────────────────────────────────────────────────────────

    def test_meta_file_has_correct_fields(self):
        meta = self._write_meta("myapp", 8080, 8080, "root@10.0.0.1", "10.0.3.10")
        data = self._parse_meta(meta)

        self.assertEqual(data["container"],    "myapp")
        self.assertEqual(data["remote_port"],  "8080")
        self.assertEqual(data["local_port"],   "8080")
        self.assertEqual(data["server"],       "root@10.0.0.1")
        self.assertEqual(data["container_ip"], "10.0.3.10")
        self.assertIn("started", data)

    def test_meta_different_local_and_remote_port(self):
        meta = self._write_meta("webapp", 8080, 9090, "deploy@server.id")
        data = self._parse_meta(meta)
        self.assertEqual(data["remote_port"], "8080")
        self.assertEqual(data["local_port"],  "9090")

    def test_tunnel_key_format(self):
        """Key = container_remoteport (dipakai sebagai prefix nama file)."""
        container   = "myapp"
        remote_port = "3000"
        key         = f"{container}_{remote_port}"
        self.assertEqual(key, "myapp_3000")

    def test_cleanup_removes_both_files(self):
        meta = self._write_meta("temp", 7000, 7000, "root@server.id")
        pid  = self._write_pid("temp", 7000, 11111)

        meta.unlink()
        pid.unlink()

        self.assertFalse(meta.exists(), "File .meta harus terhapus")
        self.assertFalse(pid.exists(),  "File .pid harus terhapus")

    def test_multiple_tunnels_have_separate_files(self):
        for container, port in [("app1", 3000), ("app2", 4000), ("app3", 5000)]:
            self._write_meta(container, port, port, "root@server.id")
            self._write_pid(container, port, 10000 + port)

        meta_files = list(self.tunnel_dir.glob("*.meta"))
        pid_files  = list(self.tunnel_dir.glob("*.pid"))

        self.assertEqual(len(meta_files), 3)
        self.assertEqual(len(pid_files),  3)
```

### Pola: Menguji State Machine Tunnel List

```python
class TestTunnelListLogic(unittest.TestCase):
    def setUp(self):
        self.tmp        = tempfile.mkdtemp(prefix="melisa_list_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_tunnel(self, container: str, remote_port: int, local_port: int,
                      server: str, pid: Optional[int] = None) -> None:
        key  = f"{container}_{remote_port}"
        meta = self.tunnel_dir / f"{key}.meta"
        meta.write_text(
            f"container={container}\n"
            f"container_ip=10.0.3.5\n"
            f"remote_port={remote_port}\n"
            f"local_port={local_port}\n"
            f"server={server}\n"
            f"started=2026-03-20 16:30:00\n"
        )
        if pid is not None:
            pid_file = self.tunnel_dir / f"{key}.pid"
            pid_file.write_text(str(pid) + "\n")

    def _is_pid_alive(self, pid: int) -> bool:
        """Mirror dari logika kill -0 di exec.sh."""
        try:
            os.kill(pid, 0)
            return True
        except (ProcessLookupError, PermissionError):
            return False

    def test_tunnel_with_no_pid_file_is_unknown(self):
        """Tunnel tanpa .pid file → status UNKNOWN."""
        self._write_tunnel("myapp", 8080, 8080, "root@10.0.0.1", pid=None)
        pid_file = self.tunnel_dir / "myapp_8080.pid"
        self.assertFalse(pid_file.exists())
        # Implementasi tunnel-list akan menandai ini sebagai UNKNOWN

    def test_tunnel_with_dead_pid_is_dead(self):
        """Tunnel dengan PID yang sudah mati → status DEAD."""
        dead_pid = 999999999  # PID yang hampir pasti tidak ada
        self._write_tunnel("myapp", 8080, 8080, "root@10.0.0.1", pid=dead_pid)
        self.assertFalse(self._is_pid_alive(dead_pid),
                         "PID 999999999 seharusnya tidak ada")

    def test_multiple_tunnels_listed_separately(self):
        self._write_tunnel("app1", 3000, 3000, "root@server.id", pid=11111)
        self._write_tunnel("app2", 4000, 4000, "root@server.id", pid=22222)

        meta_files = sorted(self.tunnel_dir.glob("*.meta"))
        self.assertEqual(len(meta_files), 2)

        names = [m.stem for m in meta_files]
        self.assertIn("app1_3000", names)
        self.assertIn("app2_4000", names)
```

---

## Pola: Menggunakan `subTest`

Untuk menguji banyak input sekaligus tanpa menulis test method terpisah per kasus:

```python
def test_valid_port_range(self):
    valid_ports = ["1", "80", "443", "8080", "65535"]
    for port in valid_ports:
        with self.subTest(port=port):
            self.assertTrue(self._validate_port(port),
                            f"Port {port!r} seharusnya valid")

def test_invalid_ports(self):
    invalid_ports = ["0", "65536", "-1", "abc", "", "3.14", "3000abc"]
    for port in invalid_ports:
        with self.subTest(port=port):
            self.assertFalse(self._validate_port(port),
                             f"Port {port!r} seharusnya ditolak")
```

`subTest` memastikan semua kasus diuji meski beberapa gagal — test tidak berhenti di kegagalan pertama.

---

## Checklist Menulis Test Baru

Sebelum menambahkan test, tanyakan:

1. **Apa yang saya uji?** Satu konsep atau satu fungsi per test method.
2. **Apakah test ini butuh infrastruktur live?** Jika ya, pertimbangkan ulang — pisahkan logika dari I/O sehingga bisa diuji secara unit.
3. **Apakah `tearDown` membersihkan semua file sementara?** Gunakan `shutil.rmtree(self.tmp, ignore_errors=True)` untuk robust cleanup.
4. **Apakah nama test method deskriptif?** `test_meta_file_has_correct_fields` lebih baik dari `test_meta`.
5. **Apakah ada pesan di setiap `assert`?** Pesan yang jelas menghemat waktu debug saat test gagal.

---

## Konvensi Penamaan

```python
# Kelas: TestNamaKomponen
class TestTunnelPortValidation(unittest.TestCase): ...
class TestTunnelFileManagement(unittest.TestCase): ...
class TestAuthProfile(unittest.TestCase): ...
class TestDbRegistry(unittest.TestCase): ...

# Method: test_apa_yang_diuji_dalam_kondisi_apa
def test_valid_port_numbers(self): ...
def test_reject_port_zero(self): ...
def test_meta_file_created_with_correct_fields(self): ...
def test_cleanup_removes_both_pid_and_meta_files(self): ...
```

---

## Menambahkan File Test Baru

Jika kamu menambahkan fitur baru di `exec.sh` atau `auth.sh`, buat file test baru jika ruang lingkupnya berbeda dari yang sudah ada:

```
src/melisa_client/ut_/
├── test_melisa.py                    ← core client logic
├── test_tunnel_and_crossregion.py    ← tunnel subsystem
└── test_<nama_fitur_baru>.py         ← tambahkan di sini
```

Struktur minimal file test baru:

```python
"""
Deskripsi singkat apa yang diuji di file ini.
"""

import unittest
import tempfile
import shutil
from pathlib import Path

# Sertakan fungsi helper yang relevan di sini atau import dari shared helper

class TestNamaFiturBaru(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_newfeature_test_")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_placeholder(self):
        self.assertTrue(True, "Ganti dengan test yang nyata")

if __name__ == "__main__":
    unittest.main()
```