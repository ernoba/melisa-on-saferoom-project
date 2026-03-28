# Security & Permissions

## File Permission Reference

| Path | Mode | Owner | Purpose |
|------|------|-------|---------|
| `/usr/local/bin/melisa` | `4755` (SUID) | `root` | Main binary — runs as root regardless of caller |
| `/etc/sudoers.d/melisa` | `0440` | `root` | Global passwordless rule for `melisa` binary |
| `/etc/sudoers.d/melisa_<user>` | `0440` | `root` | Per-user whitelist of allowed commands |
| `/opt/melisa/projects/` | `1777` (Sticky) | `root:melisa` | Master bare Git repository storage |
| `/var/lib/lxc/<n>/` | `0700` | `root` | Container root — inaccessible to non-root |
| `/var/lib/lxc/<n>/rootfs/etc/melisa-info` | `0444` | `root` | MELISA metadata — read-only |
| `/home` | `0711` | `root` | Traversable but not listable — hides user enumeration |
| `/home/<user>` | `0700` | `<user>` | Fully private home directory |
| `~/.melisa_history` | `0600` | `user` | Command history; private after `--clear` |
| `~/.config/melisa/` (client) | `700` | `user` | Profile directory |
| `~/.config/melisa/registry` (client) | `600` | `user` | Project path database |

---

## SSH Configuration (Client-Side)

MELISA's `auth add` creates `~/.ssh/sockets/` and appends to `~/.ssh/config`:

```
Host <server-ip>
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h:%p
  ControlPersist 10m
```

The `ControlPath` pattern `%r@%h:%p` expands at runtime to a Unix domain socket file. For example, a connection as `root` to `192.168.1.100:22` resolves to `~/.ssh/sockets/root@192.168.1.100:22`.

The socket is created on first connection and reused for 10 minutes. **Anyone with access to `~/.ssh/` can potentially hijack this socket.** Protect your `.ssh` directory with `chmod 700 ~/.ssh`.

> **Note for users upgrading from older installations:** Previous versions stored sockets with the naming pattern `~/.ssh/melisa_mux_%h_%p_%r` directly in `~/.ssh/`. If you have leftover files from an earlier install, remove them with:
> ```bash
> rm -f ~/.ssh/melisa_mux_*
> ```

---

## Sudoers Rule Anatomy

MELISA never grants `ALL=(ALL) NOPASSWD: ALL`. Every rule is a specific whitelist:

```
# Global rule (for the SUID binary invocation):
ALL ALL=(ALL) NOPASSWD: /usr/local/bin/melisa

# Per-user rule for Standard User alice:
alice ALL=(ALL) NOPASSWD: /usr/bin/lxc-*, /bin/lxc-*, \
  /usr/share/lxc/templates/lxc-download *, \
  /usr/bin/git *, /bin/git *, \
  /usr/local/bin/melisa *, \
  /usr/bin/mkdir -p *, /bin/mkdir -p *, \
  /usr/bin/rm -f *, /bin/rm -f *, \
  /usr/bin/bash -c *, /bin/bash -c *, \
  /usr/bin/tee *, /bin/tee *, \
  /usr/bin/chattr *, /bin/chattr *

# Per-user rule for Admin bob (adds user/group management):
bob ALL=(ALL) NOPASSWD: [all of alice's] + \
  /usr/sbin/useradd *, /sbin/useradd *, \
  /usr/sbin/userdel *, /sbin/userdel *, \
  /usr/bin/passwd *, /bin/passwd *, \
  /usr/bin/pkill *, /bin/pkill *, \
  /usr/bin/chmod *, /bin/chmod *, \
  /usr/bin/chown *, /bin/chown *, \
  /usr/bin/mkdir *, /bin/mkdir *
```

Dual paths (`/usr/bin/` and `/bin/`) ensure compatibility with both merged-usr and unmerged-usr Linux filesystem layouts (Debian/Ubuntu vs Fedora/RHEL).

---

## LXC Namespace Isolation

| Namespace | Isolation |
|-----------|-----------|
| `pid` | Container processes cannot see host PIDs |
| `net` | Container has its own network stack |
| `mnt` | Container has its own mount table |
| `uts` | Container can have its own hostname |
| `ipc` | Container IPC is isolated from host |
| `user` | Container UID 0 = host UID 100000 (unprivileged) |

The UID range `100000–165535` is allocated via `usermod --add-subuids/subgids`. This means even a "root" process inside a container has zero privileges on the host.

---

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| Remote bootstrap attack | `--setup` requires physical terminal (SSH detection) |
| Privilege escalation via MELISA | SUID binary runs controlled Rust code; sudoers whitelist is narrow |
| User enumeration | `chmod 711 /home` hides user directory listing |
| History exfiltration | History file is `0600`; cleared with TOCTOU-safe deletion |
| Container escape | LXC user namespaces map container root to unprivileged host UID |
| Path traversal in metadata | Container names validated for `/`, `\`, `..` before any filesystem op |
| Partial metadata write | Atomic write pattern: temp file → fsync → rename |
| Git cross-user repo poisoning | `safe.directory '*'` set globally; explicit per-repo trust where needed |
| SSH key interception | Ed25519 keys used (modern elliptic curve); RSA accepted as fallback |
| Stale sudoers after user deletion | `--clean` command removes orphaned `melisa_*` files |

---

## Security Recommendations

1. **Rotate SSH keys periodically.** MELISA generates keys once but doesn't manage rotation. Use `ssh-keygen` and `ssh-copy-id` manually when needed.

2. **Audit sudoers files regularly.** Run `melisa --clean` after any manual user deletions.

3. **Restrict physical access** to the host machine. The physical handshake requirement only protects the initial bootstrap — physical access to a running host bypasses all protections.

4. **Use strong passwords.** MELISA enforces password setup via `passwd` but doesn't enforce complexity rules. Consider installing `pam_pwquality`.

5. **Monitor `/opt/melisa/projects/`** for unexpected repositories. Anyone with admin access can create projects.

6. **Review shared folders** (`--share`). A host path mounted into a container with ownership `100000:100000` gives the container full read-write access to those host files.