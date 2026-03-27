# Troubleshooting

## Server Issues

### `--setup` fails: "SSH session detected"

```
[SECURITY] Remote session detected. --setup requires a physical terminal.
```

**Cause:** You ran `--setup` over an SSH connection.

**Fix:** Log in physically to the host machine's console and run the command there. This is a deliberate security requirement — see [Security Model](./concepts/security-model.md).

---

### `lxcbr0` bridge is missing

```
[ERROR] Host network bridge is down and auto-repair failed.
Tip: Run 'melisa --setup' to initialize host infrastructure.
```

**Cause:** The `lxc-net.service` service stopped or was never started.

**Fix:**
```bash
sudo systemctl start lxc-net.service
sudo systemctl enable lxc-net.service
```

Or from inside MELISA (admin), re-run setup to restore all services:
```
melisa --setup
```

---

### Container creation hangs or fails

**Symptom:** `--create` spins for a long time then times out on the network phase.

**Check 1 — Bridge exists:**
```bash
ip link show lxcbr0
```

**Check 2 — LXC services running:**
```bash
systemctl status lxc.service lxc-net.service
```

**Check 3 — Internet connectivity from host:**
```bash
curl -I https://images.linuxcontainers.org
```

**Check 4 — GPG keyring:**
```
[ERROR] GPG signature verification failed.
```
Fix: `gpg --recv-keys <key-id>` (key ID shown in error) or `sudo lxc-create -t download -- --list` to refresh the GPG ring.

---

### Container has no network / can't reach internet

**Symptom:** Container starts but `apt-get update` inside it fails.

**Check 1 — DNS is locked:**
```bash
sudo lxc-attach -n mybox -- cat /etc/resolv.conf
sudo lxc-attach -n mybox -- lsattr /etc/resolv.conf  # should show 'i' flag
```

**Check 2 — Bridge forwarding:**
```bash
sysctl net.ipv4.ip_forward  # should be 1
```
Fix: `sudo sysctl -w net.ipv4.ip_forward=1`

**Check 3 — Firewall:**
```bash
sudo firewall-cmd --list-all --zone=trusted  # lxcbr0 should be listed
```
Fix: `sudo firewall-cmd --zone=trusted --add-interface=lxcbr0 --permanent && sudo firewall-cmd --reload`

---

### `--info` returns "lacks MELISA metadata"

```
[ERROR] Container 'mybox' lacks MELISA metadata. It may not have been provisioned via the MELISA Engine.
```

**Cause:** The container was created directly with `lxc-create` outside of MELISA, or the `rootfs/etc/melisa-info` file was deleted.

**Fix (manual injection):**
```bash
sudo cat > /var/lib/lxc/mybox/rootfs/etc/melisa-info << EOF
MELISA_INSTANCE_NAME=mybox
MELISA_INSTANCE_ID=$(uuidgen)
DISTRO_SLUG=manual
DISTRO_NAME=unknown
DISTRO_RELEASE=unknown
ARCHITECTURE=amd64
CREATED_AT=$(date --iso-8601=seconds)
EOF
sudo chmod 644 /var/lib/lxc/mybox/rootfs/etc/melisa-info
```

---

### Git "dubious ownership" errors in projects

```
fatal: detected dubious ownership in repository at '/opt/melisa/projects/myapp'
```

**Cause:** Git's safe.directory check failed for the current user.

**Fix:**
```bash
sudo git config --system --add safe.directory '/opt/melisa/projects/myapp'
# Or globally:
sudo git config --system --add safe.directory '*'
```

MELISA's `--setup` configures `*` globally, so this should not occur after a complete setup.

---

### Users can't push to a project

**Symptom:** `git push origin master` fails with permission denied.

**Check 1 — User is invited:**
```
melisa --projects   # Check if the project appears in their workspace
```

**Check 2 — Group permissions on master repo:**
```bash
ls -la /opt/melisa/projects/myapp/
stat /opt/melisa/projects/myapp/objects/
```
The directory should be owned by `root:melisa` with mode `2775`.

**Fix:**
```bash
sudo chown -R root:melisa /opt/melisa/projects/myapp/
sudo chmod -R 2775 /opt/melisa/projects/myapp/
```

**Check 3 — User is in `melisa` group:**
```bash
groups <username>
```

---

### Orphaned sudoers files after manual user deletion

**Symptom:** `/etc/sudoers.d/melisa_olduser` exists but `olduser` doesn't.

**Fix:**
```
melisa --clean
```

---

## Client Issues

### "Core module is missing"

```
[FATAL ERROR] Core module 'exec.sh' is missing in /home/user/.local/share/melisa
```

**Fix:** Re-run the client installer:
```bash
cd melisa-on-saferoom-project/src/melisa_client
./install.sh
```

---

### "No active server connection found"

```
[ERROR] No active server connection found!
  Tip: Execute 'melisa auth add <n> <user@ip>' to register a server.
```

**Fix:** Register and activate a server:
```bash
melisa auth add myserver root@192.168.1.100
```

---

### SSH connection times out

**Check 1 — Server is reachable:**
```bash
ping 192.168.1.100
ssh root@192.168.1.100
```

**Check 2 — SSH key is authorized on server:**
```bash
ssh-copy-id root@192.168.1.100
```

**Check 3 — Stale multiplexing socket:**
```bash
# Current installations store sockets in ~/.ssh/sockets/
rm -f ~/.ssh/sockets/*
melisa auth add myserver root@192.168.1.100  # Re-register to recreate mux config
```

> **Note:** Older versions stored sockets with the `melisa_mux_*` naming pattern directly in `~/.ssh/`. If you have leftover files from an earlier install, clean them up with:
> ```bash
> rm -f ~/.ssh/melisa_mux_*
> ```

---

### `melisa sync` fails: "not a MELISA project"

```
[ERROR] The current directory is not registered as a MELISA project workspace.
```

**Cause:** You're in a directory that wasn't cloned via `melisa clone`.

**Fix:** Register the project manually in the registry:
```bash
# Edit ~/.config/melisa/registry
echo "myapp|$(realpath .)" >> ~/.config/melisa/registry
```

Or clone fresh:
```bash
cd ~
melisa clone myapp
```

---

### `melisa clone` creates nested directory (`myapp/myapp/`)

**Cause:** You ran `melisa clone myapp` while already inside a directory named `myapp`.

**Fix:** The anti-nesting detection checks `basename "$PWD"`. If you're inside `/projects/myapp/`, the clone correctly targets `.` (in-place). If you're inside `/projects/` and it still nests, check that the directory name matches exactly (case-sensitive).

---

## Diagnostic Commands

```bash
# Check system services (on host)
systemctl status lxc.service lxc-net.service sshd firewalld

# List all LXC containers with their IP addresses
sudo lxc-ls -P /var/lib/lxc --fancy

# Check a container's network config
sudo cat /var/lib/lxc/mybox/config

# Check LXC network quota
cat /etc/lxc/lxc-usernet

# Check subuid/subgid mappings
grep "$(whoami)" /etc/subuid /etc/subgid

# Check sudoers file for a user
sudo cat /etc/sudoers.d/melisa_alice

# Verify MELISA binary permissions (should show -rwsr-xr-x)
ls -la /usr/local/bin/melisa

# Client: show active server
cat ~/.config/melisa/active
cat ~/.config/melisa/profiles.conf
cat ~/.config/melisa/registry

# Check SSH multiplexing socket (current format)
ls -la ~/.ssh/sockets/
```