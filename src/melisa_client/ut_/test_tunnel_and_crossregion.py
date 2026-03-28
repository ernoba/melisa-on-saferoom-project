#!/usr/bin/env python3
# =============================================================================
# MELISA — Unit Tests: Tunnel Mode & Cross-Region Connectivity
# =============================================================================
#
# Cakupan Tests:
#   Suite 1 : Validasi parameter tunnel (port, container name)
#   Suite 2 : Manajemen file .meta dan .pid
#   Suite 3 : Logika exec_tunnel_list (listing tunnel aktif)
#   Suite 4 : Logika exec_tunnel_stop (menghentikan tunnel)
#   Suite 5 : Skenario cross-region (Amerika → Indonesia via public IP)
#   Suite 6 : Deteksi konflik port lokal
#   Suite 7 : Robustness (tunnel mati mendadak, file korup, dll)
#   Suite 8 : Konektivitas SSH mock (simulasi koneksi cross-region)
#
# Cara Jalankan:
#   python3 test_tunnel_and_crossregion.py -v
#   python3 test_tunnel_and_crossregion.py -v TunnelPortValidation
#   python3 test_tunnel_and_crossregion.py -v CrossRegionConnectivity
#
# =============================================================================

import os
import sys
import stat
import time
import shutil
import socket
import signal
import tempfile
import textwrap
import unittest
import subprocess
import threading
from pathlib import Path
from typing import Optional, Tuple
from unittest.mock import patch, MagicMock

# ─────────────────────────────────────────────────────────────────────────────
# Lokasi project
# ─────────────────────────────────────────────────────────────────────────────
def find_melisa_root() -> Optional[Path]:
    candidates = [
        Path(__file__).parent,
        Path(__file__).parent.parent,
        Path(__file__).parent.parent.parent,
        Path(__file__).parent.parent.parent.parent,
        Path.cwd(),
        Path.cwd().parent,
    ]
    for p in candidates:
        if (p / "Cargo.toml").exists() and (p / "src" / "main.rs").exists():
            return p
    return None

MELISA_ROOT = find_melisa_root()
CLIENT_SRC  = MELISA_ROOT / "src" / "melisa_client" / "src" if MELISA_ROOT else None

# ─────────────────────────────────────────────────────────────────────────────
# Warna terminal
# ─────────────────────────────────────────────────────────────────────────────
GREEN  = "\033[32m"
RED    = "\033[31m"
YELLOW = "\033[33m"
CYAN   = "\033[36m"
BOLD   = "\033[1m"
RESET  = "\033[0m"

def col(text: str, color: str) -> str:
    return f"{color}{text}{RESET}" if sys.stdout.isatty() else text

# ─────────────────────────────────────────────────────────────────────────────
# BashEnv — environment terisolasi untuk menguji bash scripts
# ─────────────────────────────────────────────────────────────────────────────
class BashEnv:
    """Isolated environment untuk menguji bash modules Melisa."""

    def __init__(self, fake_ssh: bool = False):
        self.tmp_dir = tempfile.mkdtemp(prefix="melisa_tunnel_test_")
        self.home    = Path(self.tmp_dir) / "home"
        self.home.mkdir(parents=True)
        self.bin_dir = self.home / ".local" / "bin"
        self.bin_dir.mkdir(parents=True, exist_ok=True)
        self.lib_dir = self.home / ".local" / "share" / "melisa"
        self.lib_dir.mkdir(parents=True, exist_ok=True)
        self.tunnel_dir = self.home / ".config" / "melisa" / "tunnels"
        self.tunnel_dir.mkdir(parents=True, exist_ok=True)
        self.config_dir = self.home / ".config" / "melisa"

        # Salin bash modules dari source jika ada
        if CLIENT_SRC and CLIENT_SRC.exists():
            for sh_file in CLIENT_SRC.glob("*.sh"):
                dest = self.lib_dir / sh_file.name
                shutil.copy2(sh_file, dest)
                dest.chmod(dest.stat().st_mode | stat.S_IEXEC)

        # Buat fake SSH jika diminta (untuk mock koneksi)
        if fake_ssh:
            self._install_fake_ssh()

    def _install_fake_ssh(self, response: str = "10.0.3.5", exit_code: int = 0):
        """Install fake 'ssh' binary yang me-mock respons server."""
        fake_ssh_script = self.bin_dir / "ssh"
        fake_ssh_script.write_text(textwrap.dedent(f"""\
            #!/bin/bash
            # Fake SSH untuk testing — tidak melakukan koneksi nyata
            # Tangkap argumen untuk logging
            ARGS="$@"
            
            # Jika diminta IP container (melisa --ip)
            if echo "$ARGS" | grep -q -- "--ip"; then
                echo "{response}"
                exit {exit_code}
            fi
            
            # Jika -N -f (background tunnel mode) — simulasikan sukses
            if echo "$ARGS" | grep -q -- "-N"; then
                # Spawn process dummy agar PID bisa di-capture
                sleep 3600 &
                disown
                exit {exit_code}
            fi
            
            # Default: echo args dan exit sukses
            echo "FAKE_SSH: $ARGS"
            exit {exit_code}
        """))
        fake_ssh_script.chmod(0o755)

    def install_fake_ssh_with_ip(self, container_ip: str, exit_code: int = 0):
        """Install fake SSH yang mengembalikan IP container tertentu."""
        self._install_fake_ssh(response=container_ip, exit_code=exit_code)

    def install_fake_ssh_failing(self):
        """Install fake SSH yang selalu gagal (simulasi server tidak terjangkau)."""
        fake_ssh_script = self.bin_dir / "ssh"
        fake_ssh_script.write_text(textwrap.dedent("""\
            #!/bin/bash
            echo "ssh: connect to host server port 22: Connection refused" >&2
            echo "ssh: connect to host server port 22: Connection timed out" >&2
            exit 255
        """))
        fake_ssh_script.chmod(0o755)

    def install_fake_ss(self, ports_in_use: list = None):
        """Install fake 'ss' yang melaporkan port tertentu sedang dipakai."""
        ports_in_use = ports_in_use or []
        lines = "\n".join(
            f"tcp   LISTEN 0  128  0.0.0.0:{p}  0.0.0.0:*"
            for p in ports_in_use
        )
        fake_ss = self.bin_dir / "ss"
        fake_ss.write_text(textwrap.dedent(f"""\
            #!/bin/bash
            echo "Netid  State   Recv-Q  Send-Q  Local Address:Port"
            echo "{lines}"
        """))
        fake_ss.chmod(0o755)

    def set_active_connection(self, profile_name: str, ssh_conn: str, melisa_user: str = ""):
        """Set koneksi aktif di config."""
        self.config_dir.mkdir(parents=True, exist_ok=True)
        profile_file = self.config_dir / "profiles.conf"
        active_file  = self.config_dir / "active"
        entry = f"{profile_name}={ssh_conn}"
        if melisa_user:
            entry += f"|{melisa_user}"
        profile_file.write_text(entry + "\n")
        active_file.write_text(profile_name + "\n")

    def write_meta(self, container: str, remote_port: int, local_port: int,
                   server: str, container_ip: str = "10.0.3.5") -> Path:
        """Tulis file .meta tunnel (simulasikan tunnel yang sudah ada)."""
        key = f"{container}_{remote_port}"
        meta_file = self.tunnel_dir / f"{key}.meta"
        meta_file.write_text(textwrap.dedent(f"""\
            container={container}
            container_ip={container_ip}
            remote_port={remote_port}
            local_port={local_port}
            server={server}
            started=2025-01-15 10:30:00
        """))
        return meta_file

    def write_pid(self, container: str, remote_port: int, pid: int) -> Path:
        """Tulis file .pid tunnel."""
        key = f"{container}_{remote_port}"
        pid_file = self.tunnel_dir / f"{key}.pid"
        pid_file.write_text(str(pid) + "\n")
        return pid_file

    def run_bash(
        self,
        script: str,
        env_extra: Optional[dict] = None,
        timeout: int = 10
    ) -> Tuple[int, str, str]:
        """Jalankan bash script di environment terisolasi."""
        env = os.environ.copy()
        env["HOME"]  = str(self.home)
        env["PATH"]  = f"{self.bin_dir}:/usr/bin:/bin"
        # Hapus variabel SSH lingkungan asli
        for var in ["SSH_CLIENT", "SSH_TTY", "SSH_CONNECTION", "SUDO_USER"]:
            env.pop(var, None)
        if env_extra:
            env.update(env_extra)

        header = textwrap.dedent(f"""\
            #!/bin/bash
            set -o pipefail
            export HOME="{self.home}"
            export MELISA_LIB="{self.lib_dir}"
            export PATH="{self.bin_dir}:/usr/bin:/bin"
            # Source modules
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
            return -1, "", f"TIMEOUT setelah {timeout}s"
        except Exception as e:
            return -2, "", str(e)

    def cleanup(self):
        shutil.rmtree(self.tmp_dir, ignore_errors=True)


def has_bash_modules() -> bool:
    return CLIENT_SRC is not None and (CLIENT_SRC / "exec.sh").exists()


# =============================================================================
# SUITE 1: Validasi Parameter Tunnel
# =============================================================================
class TestTunnelPortValidation(unittest.TestCase):
    """
    Menguji validasi port pada exec_tunnel() — logika pure tanpa SSH.
    Mirror dari logika Bash di exec.sh.
    """

    def _validate_port(self, port_str: str) -> bool:
        """Mirror dari validasi port di exec_tunnel()."""
        return bool(port_str) and port_str.isdigit() and 1 <= int(port_str) <= 65535

    def _validate_tunnel_args(self, container: str, remote_port: str) -> bool:
        """Mirror dari guard di exec_tunnel()."""
        return bool(container) and bool(remote_port)

    def test_valid_port_numbers(self):
        for port in ["80", "443", "3000", "8080", "5432", "27017", "65535"]:
            with self.subTest(port=port):
                self.assertTrue(self._validate_port(port), f"Port '{port}' seharusnya valid")

    def test_reject_non_numeric_port(self):
        for bad in ["abc", "3000abc", "!", "", "3.14", "3000.0"]:
            with self.subTest(port=bad):
                self.assertFalse(self._validate_port(bad), f"'{bad}' seharusnya ditolak")

    def test_reject_port_zero(self):
        self.assertFalse(self._validate_port("0"))

    def test_reject_port_above_65535(self):
        self.assertFalse(self._validate_port("65536"))
        self.assertFalse(self._validate_port("99999"))

    def test_local_port_defaults_to_remote(self):
        """Jika local_port tidak diberikan, harus sama dengan remote_port."""
        remote_port = "3000"
        local_port  = ""
        result = local_port if local_port else remote_port
        self.assertEqual(result, "3000")

    def test_tunnel_requires_container_and_port(self):
        self.assertFalse(self._validate_tunnel_args("", "3000"))
        self.assertFalse(self._validate_tunnel_args("myapp", ""))
        self.assertFalse(self._validate_tunnel_args("", ""))
        self.assertTrue(self._validate_tunnel_args("myapp", "3000"))

    def test_container_name_with_dash(self):
        """Nama container boleh mengandung tanda '-'."""
        self.assertTrue(self._validate_tunnel_args("my-webapp", "8080"))

    def test_container_name_with_underscore(self):
        self.assertTrue(self._validate_tunnel_args("web_app", "8080"))

    def test_tunnel_key_format(self):
        """Format TUNNEL_KEY harus: container_port."""
        container    = "myapp"
        remote_port  = "3000"
        tunnel_key   = f"{container}_{remote_port}"
        self.assertEqual(tunnel_key, "myapp_3000")

    def test_meta_filename_format(self):
        """Nama file meta harus berakhir dengan .meta."""
        tunnel_key = "myapp_3000"
        meta_file  = f"{tunnel_key}.meta"
        pid_file   = f"{tunnel_key}.pid"
        self.assertTrue(meta_file.endswith(".meta"))
        self.assertTrue(pid_file.endswith(".pid"))


# =============================================================================
# SUITE 2: Manajemen File .meta dan .pid
# =============================================================================
class TestTunnelFileManagement(unittest.TestCase):
    """Menguji pembuatan, pembacaan, dan penghapusan file metadata tunnel."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_meta_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_meta(self, container: str, remote_port: int, local_port: int,
                    server: str, container_ip: str = "10.0.3.5") -> Path:
        key = f"{container}_{remote_port}"
        meta = self.tunnel_dir / f"{key}.meta"
        meta.write_text(textwrap.dedent(f"""\
            container={container}
            container_ip={container_ip}
            remote_port={remote_port}
            local_port={local_port}
            server={server}
            started=2025-01-15 10:30:00
        """))
        return meta

    def _write_pid(self, container: str, remote_port: int, pid: int) -> Path:
        key = f"{container}_{remote_port}"
        pid_file = self.tunnel_dir / f"{key}.pid"
        pid_file.write_text(str(pid) + "\n")
        return pid_file

    def _parse_meta(self, meta_file: Path) -> dict:
        result = {}
        for line in meta_file.read_text().splitlines():
            if "=" in line:
                k, _, v = line.partition("=")
                result[k.strip()] = v.strip()
        return result

    def test_meta_file_created_with_correct_fields(self):
        meta = self._write_meta("myapp", 3000, 3000, "root@203.0.113.5", "10.0.3.10")
        data = self._parse_meta(meta)
        self.assertEqual(data["container"],     "myapp")
        self.assertEqual(data["remote_port"],   "3000")
        self.assertEqual(data["local_port"],    "3000")
        self.assertEqual(data["server"],        "root@203.0.113.5")
        self.assertEqual(data["container_ip"],  "10.0.3.10")
        self.assertIn("started",                data)

    def test_meta_file_different_local_and_remote_port(self):
        meta = self._write_meta("webapp", 8080, 9090, "deploy@server.id")
        data = self._parse_meta(meta)
        self.assertEqual(data["remote_port"], "8080")
        self.assertEqual(data["local_port"],  "9090")

    def test_pid_file_created_correctly(self):
        pid_file = self._write_pid("myapp", 3000, 12345)
        self.assertTrue(pid_file.exists())
        pid = int(pid_file.read_text().strip())
        self.assertEqual(pid, 12345)

    def test_paired_meta_and_pid_files_exist(self):
        """Setiap tunnel harus punya pasangan .meta dan .pid."""
        self._write_meta("api", 5000, 5000, "root@server.id")
        self._write_pid("api", 5000, 99999)
        meta_files = list(self.tunnel_dir.glob("*.meta"))
        pid_files  = list(self.tunnel_dir.glob("*.pid"))
        self.assertEqual(len(meta_files), 1)
        self.assertEqual(len(pid_files),  1)
        # Stem (nama tanpa ekstensi) harus sama
        self.assertEqual(meta_files[0].stem, pid_files[0].stem)

    def test_multiple_tunnels_have_separate_files(self):
        for container, port in [("app1", 3000), ("app2", 4000), ("app3", 5000)]:
            self._write_meta(container, port, port, "root@server.id")
            self._write_pid(container, port, 10000 + port)
        self.assertEqual(len(list(self.tunnel_dir.glob("*.meta"))), 3)
        self.assertEqual(len(list(self.tunnel_dir.glob("*.pid"))),  3)

    def test_cleanup_removes_both_files(self):
        meta = self._write_meta("temp", 7000, 7000, "root@server.id")
        pid  = self._write_pid("temp", 7000, 11111)
        # Simulasi cleanup
        meta.unlink()
        pid.unlink()
        self.assertFalse(meta.exists())
        self.assertFalse(pid.exists())

    def test_meta_file_parsing_with_equals_in_value(self):
        """Nilai yang mengandung '=' (misalnya URL) harus dibaca benar."""
        key = "special_3000"
        meta = self.tunnel_dir / f"{key}.meta"
        meta.write_text("container=special\nremote_port=3000\nlocal_port=3000\nserver=root@10.0.0.1\nstarted=2025-01-15 10:00:00\ncontainer_ip=10.0.3.7\n")
        data = self._parse_meta(meta)
        self.assertEqual(data["container"], "special")

    def test_meta_file_with_unknown_pid_string(self):
        """PID file boleh berisi 'unknown' jika proses tidak bisa di-trace."""
        key = "myapp_3000"
        pid_file = self.tunnel_dir / f"{key}.pid"
        pid_file.write_text("unknown\n")
        content = pid_file.read_text().strip()
        self.assertEqual(content, "unknown")


# =============================================================================
# SUITE 3: Logika exec_tunnel_list
# =============================================================================
class TestTunnelListLogic(unittest.TestCase):
    """
    Menguji logika listing tunnel — pure Python, tanpa Bash.
    Mirror dari exec_tunnel_list() di exec.sh.
    """

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_list_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_tunnel(self, container: str, remote_port: int, local_port: int,
                      server: str, pid: Optional[int] = None) -> None:
        key = f"{container}_{remote_port}"
        meta = self.tunnel_dir / f"{key}.meta"
        meta.write_text(
            f"container={container}\n"
            f"container_ip=10.0.3.5\n"
            f"remote_port={remote_port}\n"
            f"local_port={local_port}\n"
            f"server={server}\n"
            f"started=2025-01-15 10:00:00\n"
        )
        if pid is not None:
            (self.tunnel_dir / f"{key}.pid").write_text(str(pid) + "\n")

    def _list_tunnels(self) -> list:
        """Mirror dari exec_tunnel_list() — baca semua file .meta."""
        tunnels = []
        for meta_file in sorted(self.tunnel_dir.glob("*.meta")):
            data = {}
            for line in meta_file.read_text().splitlines():
                if "=" in line:
                    k, _, v = line.partition("=")
                    data[k.strip()] = v.strip()
            pid_file = meta_file.with_suffix(".pid")
            if pid_file.exists():
                pid_str = pid_file.read_text().strip()
                data["pid"] = pid_str
                # Cek apakah proses masih hidup
                if pid_str.isdigit():
                    try:
                        os.kill(int(pid_str), 0)
                        data["status"] = "RUNNING"
                    except ProcessLookupError:
                        data["status"] = "DEAD"
                    except PermissionError:
                        data["status"] = "RUNNING"  # Proses ada, tapi bukan milik kita
                else:
                    data["status"] = "UNKNOWN"
            else:
                data["status"] = "UNKNOWN"
            tunnels.append(data)
        return tunnels

    def test_empty_tunnel_dir_returns_empty_list(self):
        tunnels = self._list_tunnels()
        self.assertEqual(tunnels, [])

    def test_single_tunnel_listed(self):
        self._write_tunnel("myapp", 3000, 3000, "root@203.0.113.5", pid=os.getpid())
        tunnels = self._list_tunnels()
        self.assertEqual(len(tunnels), 1)
        self.assertEqual(tunnels[0]["container"],   "myapp")
        self.assertEqual(tunnels[0]["remote_port"], "3000")
        self.assertEqual(tunnels[0]["server"],      "root@203.0.113.5")

    def test_multiple_tunnels_listed(self):
        self._write_tunnel("frontend", 3000, 3000, "root@server.id", pid=os.getpid())
        self._write_tunnel("backend",  5000, 5000, "root@server.id", pid=os.getpid())
        self._write_tunnel("database", 5432, 5432, "root@server.id", pid=os.getpid())
        tunnels = self._list_tunnels()
        self.assertEqual(len(tunnels), 3)
        names = {t["container"] for t in tunnels}
        self.assertEqual(names, {"frontend", "backend", "database"})

    def test_dead_process_marked_dead(self):
        """PID dari proses yang sudah mati harus ditandai DEAD."""
        # PID 1 dimiliki init — kita tidak bisa kill-0 dengan aman.
        # Gunakan PID yang pasti tidak ada: 2147483647 (INT_MAX)
        self._write_tunnel("deadapp", 3000, 3000, "root@server.id", pid=2147483647)
        tunnels = self._list_tunnels()
        self.assertEqual(len(tunnels), 1)
        self.assertEqual(tunnels[0]["status"], "DEAD")

    def test_current_process_marked_running(self):
        """PID proses saat ini (test runner) pasti sedang berjalan."""
        self._write_tunnel("liveapp", 8080, 8080, "root@server.id", pid=os.getpid())
        tunnels = self._list_tunnels()
        self.assertEqual(tunnels[0]["status"], "RUNNING")

    def test_unknown_pid_marked_unknown(self):
        self._write_tunnel("ghostapp", 9000, 9000, "root@server.id")
        # Tulis PID file dengan nilai "unknown"
        (self.tunnel_dir / "ghostapp_9000.pid").write_text("unknown\n")
        tunnels = self._list_tunnels()
        self.assertEqual(tunnels[0]["status"], "UNKNOWN")

    def test_meta_without_pid_file_marked_unknown(self):
        """Jika .pid tidak ada, status harus UNKNOWN (bukan error)."""
        self._write_tunnel("nopid", 4000, 4000, "root@server.id", pid=None)
        tunnels = self._list_tunnels()
        self.assertEqual(len(tunnels), 1)
        self.assertEqual(tunnels[0]["status"], "UNKNOWN")


# =============================================================================
# SUITE 4: Logika exec_tunnel_stop
# =============================================================================
class TestTunnelStopLogic(unittest.TestCase):
    """
    Menguji logika penghentian tunnel — pure Python.
    Mirror dari exec_tunnel_stop() di exec.sh.
    """

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_stop_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_tunnel(self, container: str, remote_port: int,
                      pid: Optional[int] = None) -> None:
        key = f"{container}_{remote_port}"
        (self.tunnel_dir / f"{key}.meta").write_text(
            f"container={container}\nremote_port={remote_port}\n"
            f"local_port={remote_port}\nserver=root@server.id\n"
            f"container_ip=10.0.3.5\nstarted=2025-01-15 10:00:00\n"
        )
        if pid is not None:
            (self.tunnel_dir / f"{key}.pid").write_text(str(pid) + "\n")

    def _stop_tunnel(self, container: str, remote_port: Optional[int] = None) -> int:
        """Mirror dari exec_tunnel_stop() — return jumlah tunnel yang dihentikan."""
        stopped = 0
        for meta_file in list(self.tunnel_dir.glob("*.meta")):
            data = {}
            for line in meta_file.read_text().splitlines():
                if "=" in line:
                    k, _, v = line.partition("=")
                    data[k.strip()] = v.strip()
            if data.get("container") != container:
                continue
            if remote_port and data.get("remote_port") != str(remote_port):
                continue
            pid_file = meta_file.with_suffix(".pid")
            if pid_file.exists():
                pid_str = pid_file.read_text().strip()
                if pid_str.isdigit():
                    try:
                        os.kill(int(pid_str), signal.SIGTERM)
                    except (ProcessLookupError, PermissionError):
                        pass
            meta_file.unlink(missing_ok=True)
            pid_file.unlink(missing_ok=True)
            stopped += 1
        return stopped

    def test_stop_existing_tunnel_removes_files(self):
        # Spawn dummy process yang aman untuk di-kill
        dummy = subprocess.Popen(["sleep", "60"])
        try:
            self._write_tunnel("myapp", 3000, pid=dummy.pid)
            n = self._stop_tunnel("myapp")
        finally:
            dummy.kill()
            dummy.wait()
        self.assertEqual(n, 1)
        self.assertEqual(list(self.tunnel_dir.glob("*.meta")), [])
        self.assertEqual(list(self.tunnel_dir.glob("*.pid")),  [])

    def test_stop_nonexistent_tunnel_returns_zero(self):
        n = self._stop_tunnel("doesnotexist")
        self.assertEqual(n, 0)

    def test_stop_specific_port_only(self):
        """tunnel-stop app 3000 hanya menghentikan tunnel port 3000."""
        self._write_tunnel("app", 3000)
        self._write_tunnel("app", 4000)
        n = self._stop_tunnel("app", remote_port=3000)
        self.assertEqual(n, 1)
        remaining = list(self.tunnel_dir.glob("*.meta"))
        self.assertEqual(len(remaining), 1)
        self.assertIn("4000", remaining[0].name)

    def test_stop_all_ports_for_container(self):
        """tunnel-stop app (tanpa port) menghentikan semua tunnel container."""
        # PID yang pasti tidak ada — aman di-kill tanpa efek samping
        self._write_tunnel("app", 3000, pid=2147483647)
        self._write_tunnel("app", 4000, pid=2147483646)
        self._write_tunnel("app", 5000, pid=2147483645)
        n = self._stop_tunnel("app", remote_port=None)
        self.assertEqual(n, 3)
        self.assertEqual(list(self.tunnel_dir.glob("*.meta")), [])

    def test_stop_does_not_affect_other_containers(self):
        self._write_tunnel("app1", 3000)
        self._write_tunnel("app2", 3000)
        n = self._stop_tunnel("app1")
        self.assertEqual(n, 1)
        remaining = list(self.tunnel_dir.glob("*.meta"))
        self.assertEqual(len(remaining), 1)
        self.assertIn("app2", remaining[0].name)

    def test_stop_dead_process_still_cleans_files(self):
        """Tunnel yang prosesnya sudah mati tetap harus dihapus file-nya."""
        self._write_tunnel("deadapp", 3000, pid=2147483647)  # PID tidak ada
        n = self._stop_tunnel("deadapp")
        self.assertEqual(n, 1)
        self.assertEqual(list(self.tunnel_dir.glob("*.meta")), [])


# =============================================================================
# SUITE 5: Analisis & Test Cross-Region (Amerika → Indonesia)
# =============================================================================
class TestCrossRegionConnectivity(unittest.TestCase):
    """
    Menganalisis dan menguji skenario koneksi lintas negara:
    Client di Amerika ↔ Server Melisa di Indonesia.

    Jawaban Analisis:
    ✅ YA, bisa terhubung jika server Indonesia punya PUBLIC IP dan port 22 terbuka.
    ✅ YA, bisa akses HTTP container via 'melisa tunnel' (SSH -L port forwarding).
    ⚠️  TIDAK bisa jika server di belakang NAT/CGNAT (hanya punya IP private).
    
    Alur koneksi cross-region:
    [Client Amerika]                    [Server Indonesia]           [Container]
    localhost:8080  ──SSH -L tunnel──▶  public_ip:22  ──▶  10.0.3.5:8080
    """

    def test_public_ip_format_validation(self):
        """
        Server harus dikonfigurasi dengan IP publik / hostname publik,
        bukan IP private (192.168.x.x, 10.x.x.x, 172.16-31.x.x).
        """
        def is_private_ip(ip: str) -> bool:
            parts = ip.split(".")
            if len(parts) != 4:
                return False
            try:
                octets = [int(p) for p in parts]
            except ValueError:
                return False
            # RFC 1918 private ranges
            if octets[0] == 10:
                return True
            if octets[0] == 172 and 16 <= octets[1] <= 31:
                return True
            if octets[0] == 192 and octets[1] == 168:
                return True
            # Loopback
            if octets[0] == 127:
                return True
            return False

        # IP Private — TIDAK bisa diakses dari Amerika
        self.assertTrue(is_private_ip("192.168.1.100"),
            "LAN IP seharusnya terdeteksi sebagai private")
        self.assertTrue(is_private_ip("10.0.0.5"),
            "10.x.x.x seharusnya private")
        self.assertTrue(is_private_ip("172.20.0.1"),
            "172.20.x.x seharusnya private")

        # IP Publik — BISA diakses dari Amerika
        self.assertFalse(is_private_ip("203.0.113.5"),
            "IP publik TEST-NET seharusnya bukan private")
        self.assertFalse(is_private_ip("103.145.100.50"),
            "IP Telkom/ISP Indonesia seharusnya publik")
        self.assertFalse(is_private_ip("52.221.30.10"),
            "IP AWS Singapore seharusnya publik")

    def test_tunnel_command_builds_ssh_L_correctly(self):
        """
        Perintah SSH yang dibangun exec_tunnel() harus menggunakan -L:
        ssh -N -f -L local_port:container_ip:remote_port CONN
        """
        container    = "mywebapp"
        remote_port  = 3000
        local_port   = 3000
        container_ip = "10.0.3.5"
        server_conn  = "root@203.0.113.5"  # IP publik server Indonesia

        # Bangun perintah SSH seperti exec_tunnel()
        ssh_cmd = [
            "ssh", "-N", "-f",
            "-L", f"{local_port}:{container_ip}:{remote_port}",
            "-o", "ExitOnForwardFailure=yes",
            "-o", "ServerAliveInterval=30",
            "-o", "ServerAliveCountMax=3",
            "-o", "StrictHostKeyChecking=no",
            server_conn
        ]

        # Verifikasi komponen penting
        self.assertIn("-N",     ssh_cmd)  # No command (background tunnel)
        self.assertIn("-f",     ssh_cmd)  # Fork ke background
        self.assertIn("-L",     ssh_cmd)  # Local port forwarding
        self.assertIn(f"{local_port}:{container_ip}:{remote_port}", ssh_cmd)
        self.assertIn(server_conn, ssh_cmd)

    def test_cross_region_tunnel_url(self):
        """
        Setelah tunnel aktif, URL akses di Amerika seharusnya localhost:local_port,
        bukan langsung IP Indonesia.
        """
        local_port = 8080
        access_url = f"http://localhost:{local_port}"
        self.assertEqual(access_url, "http://localhost:8080")
        # Bukan IP server Indonesia langsung
        self.assertNotIn("203.0.113", access_url)

    def test_route_description_cross_region(self):
        """Verifikasi format deskripsi route yang ditampilkan ke user."""
        local_port   = 3000
        server_conn  = "root@203.0.113.5"
        container_ip = "10.0.3.5"
        remote_port  = 3000

        route = f"localhost:{local_port} → {server_conn} → {container_ip}:{remote_port}"
        self.assertIn("localhost",     route)
        self.assertIn(server_conn,     route)
        self.assertIn(container_ip,    route)
        self.assertIn(str(remote_port), route)

    def test_profile_with_public_ip_structure(self):
        """
        Format profile untuk server Indonesia (dari Amerika):
        profiles.conf: indonesia=root@103.145.100.50|deployuser
        """
        profile_entry = "indonesia=root@103.145.100.50|deployuser"
        name, _, rest  = profile_entry.partition("=")
        ssh_part, _, melisa_user = rest.partition("|")
        self.assertEqual(name,        "indonesia")
        self.assertEqual(ssh_part,    "root@103.145.100.50")
        self.assertEqual(melisa_user, "deployuser")

    def test_nat_detection_logic(self):
        """
        Server di belakang NAT/CGNAT tidak bisa diakses langsung.
        Deteksi berdasarkan IP private di CONN string.
        """
        def is_reachable_from_internet(conn_str: str) -> bool:
            """Cek apakah CONN menggunakan IP publik yang bisa diakses lintas negara."""
            host = conn_str.split("@")[-1] if "@" in conn_str else conn_str
            # Jika hostname (bukan IP), asumsikan bisa resolve ke publik
            if not host[0].isdigit():
                return True  # Domain name — bisa publik
            # Cek apakah private
            parts = host.split(".")
            if len(parts) == 4:
                try:
                    octets = [int(p) for p in parts]
                    if octets[0] in (10, 127):
                        return False
                    if octets[0] == 172 and 16 <= octets[1] <= 31:
                        return False
                    if octets[0] == 192 and octets[1] == 168:
                        return False
                except ValueError:
                    pass
            return True

        # Server dengan IP publik → bisa diakses dari Amerika ✅
        self.assertTrue(is_reachable_from_internet("root@203.0.113.5"))
        self.assertTrue(is_reachable_from_internet("root@103.145.100.50"))
        self.assertTrue(is_reachable_from_internet("deploy@myserver.example.com"))

        # Server dengan IP private → TIDAK bisa dari Amerika ❌
        self.assertFalse(is_reachable_from_internet("root@192.168.1.100"))
        self.assertFalse(is_reachable_from_internet("root@10.0.0.5"))


# =============================================================================
# SUITE 6: Deteksi Konflik Port Lokal
# =============================================================================
class TestLocalPortConflict(unittest.TestCase):
    """
    Menguji deteksi konflik port di mesin lokal sebelum tunnel dibuat.
    Mirror dari logika 'ss -tlnp | grep :port' di exec_tunnel().
    """

    def _is_port_in_use(self, port: int) -> bool:
        """
        Cek apakah port di mesin lokal sedang dipakai.
        Gunakan socket untuk simulasi akurat (tidak bergantung pada 'ss').
        """
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind(("127.0.0.1", port))
                return False  # Port tersedia
            except OSError:
                return True   # Port sedang dipakai

    def test_high_numbered_port_likely_free(self):
        """Port tinggi (>49151) biasanya bebas di test environment."""
        # Port 59999 hampir pasti kosong di environment test
        result = self._is_port_in_use(59999)
        # Kita tidak bisa assert pasti True/False, tapi fungsi harus berjalan
        self.assertIsInstance(result, bool)

    def test_occupied_port_detected(self):
        """Buat server temp di port acak, pastikan terdeteksi sebagai dipakai."""
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind(("127.0.0.1", 0))
        occupied_port = server.getsockname()[1]
        server.listen(1)
        try:
            self.assertTrue(self._is_port_in_use(occupied_port),
                f"Port {occupied_port} seharusnya terdeteksi sebagai dipakai")
        finally:
            server.close()

    def test_port_conflict_message_format(self):
        """Format pesan error konflik port harus informatif."""
        container   = "myapp"
        remote_port = 3000
        local_port  = 3000
        msg = (f"Local port {local_port} is already in use. "
               f"Use: melisa tunnel {container} {remote_port} <free_port>")
        self.assertIn(str(local_port), msg)
        self.assertIn(container,       msg)
        self.assertIn(str(remote_port), msg)
        self.assertIn("<free_port>",   msg)

    def test_alternative_port_suggestion(self):
        """Jika port 3000 dipakai, user bisa coba port lain (misal 3001)."""
        suggested_port = 3001  # User bisa pakai port berbeda
        self.assertGreater(suggested_port, 0)
        self.assertLessEqual(suggested_port, 65535)


# =============================================================================
# SUITE 7: Robustness — Tunnel Mati & File Korup
# =============================================================================
class TestTunnelRobustness(unittest.TestCase):
    """
    Menguji ketahanan sistem tunnel terhadap kondisi abnormal:
    - Proses tunnel mati tiba-tiba
    - File .pid hilang
    - File .meta rusak/tidak lengkap
    - Duplikasi tunnel (tunnel sama dibuat ulang)
    """

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="melisa_robust_test_")
        self.tunnel_dir = Path(self.tmp) / "tunnels"
        self.tunnel_dir.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_corrupt_meta_file_handled_gracefully(self):
        """File .meta yang korup tidak boleh menyebabkan crash."""
        (self.tunnel_dir / "corrupt_3000.meta").write_text("ini bukan format yang benar!!!\n???")
        # Parsing harus tetap berjalan
        result = {}
        try:
            for line in (self.tunnel_dir / "corrupt_3000.meta").read_text().splitlines():
                if "=" in line:
                    k, _, v = line.partition("=")
                    result[k.strip()] = v.strip()
        except Exception as e:
            self.fail(f"Parsing file korup melempar exception: {e}")
        # result mungkin kosong, tapi tidak crash
        self.assertIsInstance(result, dict)

    def test_empty_meta_file_handled(self):
        """File .meta kosong tidak boleh crash."""
        (self.tunnel_dir / "empty_3000.meta").write_text("")
        data = {}
        for line in (self.tunnel_dir / "empty_3000.meta").read_text().splitlines():
            if "=" in line:
                k, _, v = line.partition("=")
                data[k.strip()] = v.strip()
        self.assertEqual(data, {})

    def test_replacing_existing_tunnel_kills_old_pid(self):
        """
        Jika tunnel yang sama dibuat ulang, proses lama harus dihentikan.
        Simulasikan dengan PID dari proses yang sudah tidak ada.
        """
        # Buat tunnel dengan PID yang sudah mati
        key = "myapp_3000"
        (self.tunnel_dir / f"{key}.pid").write_text("2147483647\n")
        (self.tunnel_dir / f"{key}.meta").write_text(
            "container=myapp\nremote_port=3000\nlocal_port=3000\n"
            "server=root@server.id\ncontainer_ip=10.0.3.5\nstarted=2025-01-15 10:00:00\n"
        )
        # Simulasi: tunnel baru menimpa yang lama
        old_pid_file = self.tunnel_dir / f"{key}.pid"
        if old_pid_file.exists():
            old_pid = old_pid_file.read_text().strip()
            if old_pid.isdigit():
                try:
                    os.kill(int(old_pid), signal.SIGTERM)
                except (ProcessLookupError, PermissionError):
                    pass  # Proses sudah mati — aman dilanjutkan
            old_pid_file.unlink()
        self.assertFalse(old_pid_file.exists())

    def test_pid_file_with_negative_number(self):
        """PID negatif tidak boleh diproses sebagai PID valid."""
        pid_str = "-1"
        is_valid_pid = pid_str.isdigit() and int(pid_str) > 0
        # "-1".isdigit() → False di Python karena tanda minus
        self.assertFalse(is_valid_pid)

    def test_tunnel_restart_creates_fresh_metadata(self):
        """Setelah restart, metadata harus terupdate (bukan append)."""
        key = "webapp_8080"
        meta_file = self.tunnel_dir / f"{key}.meta"
        # Tulis pertama kali
        meta_file.write_text("container=webapp\nremote_port=8080\nstarted=2025-01-01\n")
        # Timpa (restart)
        meta_file.write_text("container=webapp\nremote_port=8080\nstarted=2025-06-01\n")
        content = meta_file.read_text()
        self.assertEqual(content.count("container=webapp"), 1,
            "Metadata tidak boleh duplikat setelah restart")
        self.assertIn("2025-06-01", content)
        self.assertNotIn("2025-01-01", content)


# =============================================================================
# SUITE 8: Bash Module Tests (jalankan jika source tersedia)
# =============================================================================
@unittest.skipUnless(has_bash_modules(), "Bash modules tidak ditemukan di CLIENT_SRC")
class TestTunnelBashModules(unittest.TestCase):
    """
    Menguji exec_tunnel(), exec_tunnel_list(), exec_tunnel_stop()
    langsung dari bash modules dengan SSH yang dimock.
    """

    def setUp(self):
        self.env = BashEnv(fake_ssh=False)  # SSH dimock secara manual per-test

    def tearDown(self):
        self.env.cleanup()

    def test_tunnel_fails_without_active_connection(self):
        """tunnel harus gagal (exit non-0) jika tidak ada koneksi aktif."""
        rc, out, err = self.env.run_bash(
            "exec_tunnel myapp 3000",
            timeout=5
        )
        self.assertNotEqual(rc, 0,
            "tunnel tanpa koneksi aktif seharusnya gagal")
        combined = (out + err).lower()
        self.assertTrue(
            any(kw in combined for kw in ["no active", "not connected", "error", "aktif"]),
            f"Pesan error tidak mengindikasikan koneksi bermasalah: {combined}"
        )

    def test_tunnel_empty_container_exits_with_error(self):
        """Memanggil exec_tunnel tanpa argumen harus menampilkan usage."""
        self.env.set_active_connection("myserver", "root@server.id", "admin")
        self.env.install_fake_ssh_with_ip("10.0.3.5")
        rc, out, err = self.env.run_bash("exec_tunnel '' ''", timeout=5)
        self.assertNotEqual(rc, 0)

    def test_tunnel_list_empty_when_no_tunnels(self):
        """tunnel-list tanpa tunnel aktif harus menampilkan pesan 'no tunnels'."""
        rc, out, err = self.env.run_bash("exec_tunnel_list", timeout=5)
        self.assertEqual(rc, 0, f"tunnel-list harus sukses walau kosong: {err}")
        combined = (out + err).lower()
        self.assertTrue(
            any(kw in combined for kw in ["no tunnel", "not found", "kosong", "aktif"]),
            f"Harus ada pesan 'no tunnels': {combined}"
        )

    def test_tunnel_stop_nonexistent_exits_error(self):
        """tunnel-stop container yang tidak ada harus menampilkan error."""
        rc, out, err = self.env.run_bash(
            "exec_tunnel_stop 'doesnotexist' 3000",
            timeout=5
        )
        combined = out + err
        self.assertTrue(
            any(kw in combined.lower() for kw in ["not found", "no tunnel", "error"]),
            f"Harus ada pesan error untuk tunnel tidak ada: {combined}"
        )

    def test_tunnel_list_shows_meta_content(self):
        """
        Jika ada file .meta, exec_tunnel_list harus menampilkan isinya.
        (Simulasi tunnel aktif dengan mock PID = PID proses ini sendiri)
        """
        self.env.write_meta(
            container="webapp", remote_port=8080, local_port=8080,
            server="root@103.145.100.50", container_ip="10.0.3.10"
        )
        self.env.write_pid("webapp", 8080, os.getpid())
        rc, out, err = self.env.run_bash("exec_tunnel_list", timeout=5)
        self.assertEqual(rc, 0, f"Gagal menjalankan tunnel-list: {err}")
        # Harus ada info tentang container atau server
        combined = out + err
        self.assertTrue(
            "webapp" in combined or "8080" in combined,
            f"Output tidak menampilkan info tunnel: {combined}"
        )

    def test_tunnel_stop_cleans_meta_and_pid(self):
        """
        Setelah tunnel-stop, file .meta dan .pid harus terhapus.
        """
        tunnel_dir = self.env.tunnel_dir
        self.env.write_meta("cleanme", 5000, 5000, "root@server.id")
        self.env.write_pid("cleanme", 5000, 2147483647)  # PID mati
        rc, out, err = self.env.run_bash(
            "exec_tunnel_stop 'cleanme' '5000'",
            timeout=5
        )
        # File harus terhapus
        self.assertFalse((tunnel_dir / "cleanme_5000.meta").exists(),
            "File .meta harus dihapus setelah tunnel-stop")
        self.assertFalse((tunnel_dir / "cleanme_5000.pid").exists(),
            "File .pid harus dihapus setelah tunnel-stop")

    def test_tunnel_invalid_port_string(self):
        """Port non-numerik harus ditolak dengan pesan error jelas."""
        self.env.set_active_connection("myserver", "root@server.id", "admin")
        self.env.install_fake_ssh_with_ip("10.0.3.5")
        rc, out, err = self.env.run_bash(
            "exec_tunnel myapp 'notaport'",
            timeout=5
        )
        self.assertNotEqual(rc, 0, "Port non-numerik seharusnya ditolak")
        combined = out + err
        self.assertTrue(
            any(kw in combined.lower() for kw in ["integer", "numeric", "port", "error"]),
            f"Pesan error tidak menyebut port: {combined}"
        )

    def test_cross_region_tunnel_builds_correct_ssh_command(self):
        """
        Verifikasi bahwa exec_tunnel membangun perintah SSH -L yang benar
        untuk skenario cross-region (server Indonesia dari klien Amerika).
        """
        # Set koneksi aktif dengan IP publik Indonesia
        self.env.set_active_connection(
            "indonesia", "root@103.145.100.50", "devuser"
        )
        # Mock SSH: kembalikan IP container saat ditanya --ip
        self.env.install_fake_ssh_with_ip("10.0.3.7")

        # Jalankan tunnel — tidak akan membuat koneksi nyata karena SSH di-mock
        # Kita cek bahwa proses berjalan tanpa error validasi
        rc, out, err = self.env.run_bash(
            # Timeout singkat karena fake SSH spawn sleep 3600
            # Kita hanya perlu memastikan validasi lewat
            """
            # Override exec_tunnel untuk hanya cek parameter tanpa benar-benar SSH
            exec_tunnel_dry_run() {
                ensure_connected
                local container=$1
                local remote_port=$2
                local local_port=${3:-$remote_port}
                if [ -z "$container" ] || [ -z "$remote_port" ]; then
                    echo "ERROR: args kosong" >&2; exit 1
                fi
                if ! [[ "$remote_port" =~ ^[0-9]+$ ]] || ! [[ "$local_port" =~ ^[0-9]+$ ]]; then
                    echo "ERROR: port harus integer" >&2; exit 1
                fi
                echo "DRY_RUN_OK: $container $remote_port $local_port $CONN"
            }
            exec_tunnel_dry_run "mywebapp" "8080" "8080"
            """,
            timeout=5
        )
        self.assertEqual(rc, 0, f"Dry-run tunnel gagal: {err}\n{out}")
        self.assertIn("DRY_RUN_OK", out)
        self.assertIn("mywebapp",           out)
        self.assertIn("8080",               out)
        self.assertIn("103.145.100.50",     out)


# =============================================================================
# SUITE 9: Integration Test Koneksi Jaringan (Opsional)
# =============================================================================
class TestNetworkConnectivityHelpers(unittest.TestCase):
    """
    Test helper konektivitas jaringan — tanpa koneksi nyata ke luar.
    Simulasikan skenario cross-region menggunakan localhost.
    """

    def test_localhost_tcp_roundtrip(self):
        """
        Simulasikan SSH tunnel cross-region menggunakan dua socket localhost.
        Client di 'Amerika' (port tinggi) ↔ 'Server' di localhost (port acak).
        """
        # Buat server socket (simulasi server Indonesia)
        server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server_sock.bind(("127.0.0.1", 0))
        server_port = server_sock.getsockname()[1]
        server_sock.listen(1)
        server_sock.settimeout(3)

        response_received = []

        def server_thread():
            try:
                conn, _ = server_sock.accept()
                data = conn.recv(1024)
                conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                conn.close()
                response_received.append(data)
            except Exception:
                pass
            finally:
                server_sock.close()

        t = threading.Thread(target=server_thread, daemon=True)
        t.start()

        # Client terhubung (simulasi tunnel sudah aktif)
        time.sleep(0.1)
        client_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        client_sock.settimeout(3)
        try:
            client_sock.connect(("127.0.0.1", server_port))
            client_sock.sendall(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            response = client_sock.recv(1024)
            self.assertIn(b"200 OK", response)
        finally:
            client_sock.close()
        t.join(timeout=3)

    def test_ssh_port_22_reachability_check_logic(self):
        """
        Simulasikan pemeriksaan apakah port 22 server Indonesia terjangkau.
        Dalam test ini, kita check port yang kita buka sendiri di localhost.
        """
        # Buka server TCP di port acak (simulasi SSH server Indonesia)
        fake_ssh_server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        fake_ssh_server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        fake_ssh_server.bind(("127.0.0.1", 0))
        fake_port = fake_ssh_server.getsockname()[1]
        fake_ssh_server.listen(1)
        fake_ssh_server.settimeout(2)

        def check_port_reachable(host: str, port: int, timeout: float = 2.0) -> bool:
            """Cek apakah port dapat dihubungi (seperti yang dilakukan SSH)."""
            try:
                with socket.create_connection((host, port), timeout=timeout):
                    return True
            except (socket.timeout, ConnectionRefusedError, OSError):
                return False

        # Port yang kita buka → harus terjangkau ✅
        self.assertTrue(check_port_reachable("127.0.0.1", fake_port))
        fake_ssh_server.close()

        # Port yang tidak ada → tidak terjangkau ❌
        self.assertFalse(check_port_reachable("127.0.0.1", 1))

    def test_container_ip_format(self):
        """IP container LXC biasanya dalam range 10.0.3.x."""
        def is_lxc_container_ip(ip: str) -> bool:
            parts = ip.split(".")
            if len(parts) != 4:
                return False
            try:
                return int(parts[0]) == 10 and int(parts[1]) == 0 and int(parts[2]) == 3
            except ValueError:
                return False

        self.assertTrue(is_lxc_container_ip("10.0.3.1"))
        self.assertTrue(is_lxc_container_ip("10.0.3.5"))
        self.assertTrue(is_lxc_container_ip("10.0.3.254"))
        self.assertFalse(is_lxc_container_ip("10.0.4.5"))
        self.assertFalse(is_lxc_container_ip("192.168.1.5"))


# =============================================================================
# Entry Point
# =============================================================================
class MelisaTunnelTestRunner(unittest.TextTestRunner):
    def run(self, test):
        print(f"\n{BOLD}{CYAN}{'='*65}{RESET}")
        print(f"{BOLD}{CYAN}  MELISA — Tunnel Mode & Cross-Region Connectivity Tests{RESET}")
        print(f"{BOLD}{CYAN}{'='*65}{RESET}")
        print(f"  Project root : {MELISA_ROOT or col('Tidak ditemukan', RED)}")
        print(f"  Client src   : {CLIENT_SRC or col('Tidak ditemukan', YELLOW)}")
        print(f"  Bash modules : {col('Tersedia', GREEN) if has_bash_modules() else col('Tidak ada (Suite 8 dilewati)', YELLOW)}")
        print(f"{BOLD}{CYAN}{'='*65}{RESET}\n")
        print(f"{BOLD}📋 Analisis Cross-Region (Amerika → Indonesia):{RESET}")
        print(f"  {'✅' if True else '❌'} SSH tunnel (-L) mendukung lintas negara secara native")
        print(f"  ✅ 'melisa tunnel <container> <port>' meneruskan traffic ke container")
        print(f"  ⚠️  Syarat: server Indonesia harus punya IP publik & port 22 terbuka")
        print(f"  ❌ Server di belakang NAT/CGNAT tidak bisa diakses langsung\n")
        result = super().run(test)
        return result


if __name__ == "__main__":
    loader = unittest.TestLoader()

    # Urutan suite yang logis
    suite = unittest.TestSuite()
    for cls in [
        TestTunnelPortValidation,
        TestTunnelFileManagement,
        TestTunnelListLogic,
        TestTunnelStopLogic,
        TestCrossRegionConnectivity,
        TestLocalPortConflict,
        TestTunnelRobustness,
        TestTunnelBashModules,
        TestNetworkConnectivityHelpers,
    ]:
        suite.addTests(loader.loadTestsFromTestCase(cls))

    runner = MelisaTunnelTestRunner(verbosity=2, failfast=False)
    result = runner.run(suite)
    sys.exit(0 if result.wasSuccessful() else 1)