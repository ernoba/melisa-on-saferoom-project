# Authentication & Connections

The `auth` command group manages your remote server profiles: which servers exist, which one is active, and the SSH configuration that connects you to them.

---

## `melisa auth add <n> <user@host>`

Registers a new remote MELISA server profile. This is the **first command** you run after installing the client.

```bash
melisa auth add myserver root@192.168.1.100
```

Replace `myserver` with any nickname and `root@192.168.1.100` with your actual server address.

### What `auth add` Does — Step by Step

**1. SSH Key Check**

Checks for `~/.ssh/id_ed25519` or `~/.ssh/id_rsa`. If neither exists, generates a modern **Ed25519 keypair** automatically:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N "" -q
```

Generates a passphrase-less Ed25519 keypair (required for automated CLI operations without password prompts).

**2. Public Key Distribution**

```bash
ssh-copy-id -i ~/.ssh/id_ed25519.pub root@192.168.1.100
```

You'll be prompted for the **server's password once**. After this, all subsequent connections use key authentication.

**3. SSH Multiplexing Configuration**

Creates `~/.ssh/sockets/` and appends to `~/.ssh/config`:

```
Host 192.168.1.100
  User root
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h:%p
  ControlPersist 10m
```

`ControlMaster auto` keeps a single master SSH connection alive for 10 minutes after the last use. All subsequent commands reuse this connection via the socket at `~/.ssh/sockets/`, making commands nearly instantaneous instead of incurring SSH handshake overhead on every call.

**4. MELISA Username Prompt**

After key setup, the client asks for your **MELISA application username** on the remote server. This is the username you log in with inside the MELISA environment — it may differ from your SSH login user (e.g., you SSH as `root` but your MELISA identity is `alice`):

```
[SETUP] Enter your MELISA username on this server (leave blank if same as SSH user):
```

Leave it blank to use the SSH login user as both the transport and the MELISA identity. The client stores this separately so it can correctly resolve project paths (e.g., `/home/alice/myapp/` vs. `/home/root/myapp/`).

**5. Profile Storage**

Appends to `~/.config/melisa/profiles.conf` in a pipe-extended key-value format:

```
myserver=root@192.168.1.100|alice
```

The format is `name=ssh_connection|melisa_username`. The SSH connection (`root@192.168.1.100`) is used for transport; the MELISA username (`alice`) is used to resolve workspace paths on the server. If the MELISA username is the same as the SSH user, it is still stored for explicitness.

**6. Auto-activation**

Writes `myserver` to `~/.config/melisa/active`, making it the default for all future commands.

---

## `melisa auth switch <n>`

Changes the active server without re-configuring anything:

```bash
melisa auth switch production
```

```
[SUCCESS] Successfully switched active connection to server: production
```

Validates that the profile name exists in `profiles.conf` before writing to `active`. If not found:

```
[ERROR] Server profile 'typo' not found! Execute 'melisa auth list' to view available profiles.
```

---

## `melisa auth list`

Displays all registered servers with a clear active marker:

```bash
melisa auth list
```

```
=== MELISA REMOTE SERVER REGISTRY ===
  * myserver      (root@192.168.1.100) [melisa: alice]  <- [ACTIVE]
    production    (deploy@prod.example.com)
    homelab       (alice@10.0.0.5)
```

The `*` prefix and `<- [ACTIVE]` suffix mark the currently active server. The optional `[melisa: <user>]` tag is shown when the stored MELISA username differs from the SSH user. Reads directly from `profiles.conf`.

---

## `melisa auth remove <n>`

Unregisters a server profile:

```bash
melisa auth remove homelab
```

Removes the `homelab=...` line from `profiles.conf`. Does **not** remove the SSH key or the multiplexing socket — only the profile registration.

If you remove the currently active server, you'll need to run `auth switch` to set a new active server before issuing any commands.

---

## Profile Files Reference

### `~/.config/melisa/profiles.conf`

Plain key-value store, one profile per line. Each line stores both the SSH transport and the MELISA application identity:

```
myserver=root@192.168.1.100|alice
production=deploy@prod.example.com|deploy
homelab=alice@10.0.0.5|alice
```

Format: `name=user@host|melisa_username`

> **Note for profiles registered before v0.1.2:** Older entries in the format `name=user@host` (without the pipe) are still supported. The client falls back to using the SSH username as the MELISA identity in that case.

### `~/.config/melisa/active`

Single line containing the active profile name:

```
myserver
```

### `~/.config/melisa/registry`

Pipe-delimited project path mappings (managed automatically by `clone` and `get`):

```
myapp|/home/user/projects/myapp
backend|/home/user/work/backend
```

`sync` reads this to locate your project root.

---

## Resolving the Active Connection (Internal)

Three internal functions in `auth.sh` resolve the active connection:

**`get_active_conn`** — returns only the SSH connection string (`user@host`), stripping the MELISA username portion:

```bash
get_active_conn() {
    if [ ! -f "$ACTIVE_FILE" ]; then return 1; fi
    local active
    active=$(cat "$ACTIVE_FILE")

    local entry
    entry=$(grep "^${active}=" "$PROFILE_FILE" | cut -d'=' -f2)

    # Strip the "|melisa_user" suffix — return ONLY the "user@host" part
    local conn
    conn=$(echo "$entry" | cut -d'|' -f1)

    if [ -z "$conn" ]; then return 1; fi
    echo "$conn"
}
```

Returns empty / exit code 1 if no active profile is set, which causes `ensure_connected` to abort with an error and the tip to add a server.

**`get_remote_user`** — returns only the MELISA username (the part after `|`):

```bash
get_remote_user() {
    # Reads the full stored value and extracts the part after "|"
    # Returns empty string if no "|" separator exists (legacy profile)
    echo "$raw" | cut -s -d'|' -f2
}
```

**`get_active_melisa_user`** — returns the MELISA username, falling back to the SSH user if not set:

```bash
get_active_melisa_user() {
    # Returns the melisa_user stored after "|"
    # If none was stored, falls back to the SSH login username (before "@")
}
```

These three functions are used by `exec.sh` to correctly route SSH transport and workspace path resolution independently, which matters when you SSH as `root` but your projects live in `/home/alice/`.