# SSH Tunnels

The MELISA tunnel subsystem creates persistent SSH port-forward tunnels from your local workstation directly to a port inside a remote container. This works transparently across different networks — as long as the server's SSH port is reachable, any port inside any container becomes available on `localhost`.

---

## `melisa tunnel <container> <remote_port> [local_port]`

Opens a background SSH tunnel forwarding a container port to your local machine.

```bash
# Forward container port 8080 to localhost:8080
melisa tunnel myapp 8080

# Forward to a different local port (if 8080 is already in use)
melisa tunnel myapp 8080 9090
```

**Output:**

```
━━━ SSH Tunnel Setup: myapp ━━━
[INFO] Querying container IP from server 'root@192.168.1.100'...
  Container IP : 10.0.3.5
[INFO] Establishing tunnel: localhost:8080 → [root@192.168.1.100] → 10.0.3.5:8080
[SUCCESS] Tunnel active!

  ► ACCESS URL  :  http://localhost:8080
  ► ROUTE       :  localhost:8080 → root@192.168.1.100 → 10.0.3.5:8080
  ► PID         :  12345
  ► STOP WITH   :  melisa tunnel-stop myapp 8080

  [NOTE] This tunnel works across different networks as long as
         the SSH port of 'root@192.168.1.100' is reachable from this machine.
```

### How It Works

```
Your Machine          MELISA Host           Container
────────────          ────────────          ─────────
localhost:8080   →    192.168.1.100   →    10.0.3.5:8080
    (SSH -L)               (SSH)           (container net)
```

1. Queries `melisa --ip <container>` on the server to resolve the container's internal bridge IP
2. Checks that `local_port` is not already in use (`ss -tlnp`)
3. If a previous tunnel for the same `container+port` exists, kills it first
4. Spawns `ssh -N -f -L local_port:container_ip:remote_port` as a background process
5. Saves the process PID and tunnel metadata to `~/.config/melisa/tunnels/`

### Pre-flight Checks

The command fails with a clear error if:
- No active server connection is registered (`melisa auth add` first)
- The container name or port arguments are missing
- Port numbers are not valid integers
- The local port is already in use (suggests an alternative: `melisa tunnel <cont> <rport> <free_port>`)
- The container IP cannot be resolved (container may be stopped — run `melisa --run <container>` first)

### Keepalive Settings

The tunnel uses `ServerAliveInterval=30` and `ServerAliveCountMax=3`. If the server goes silent for 90 seconds, the SSH process exits. Use `melisa tunnel-list` to check the status and re-establish if needed.

---

## `melisa tunnel-list`

Displays all active tunnels with their current status.

```bash
melisa tunnel-list
```

```
━━━ Active Tunnels ━━━
  CONTAINER            R.PORT   L.PORT   SERVER                    STATUS
  ────────────────────────────────────────────────────────────────────────
  myapp                8080     8080     root@192.168.1.100        RUNNING (PID 12345)
  [INFO] Access: http://localhost:8080  |  Started: 2026-03-20 16:30:00

  devlab               5432     5432     root@192.168.1.100        RUNNING (PID 12346)
  [INFO] Access: http://localhost:5432  |  Started: 2026-03-20 17:00:00
```

**Status values:**

| Status | Meaning |
|--------|---------|
| `RUNNING (PID N)` | The SSH process is alive and the tunnel is active |
| `DEAD` | The PID no longer exists — the tunnel has died. The entry is automatically removed. |
| `UNKNOWN` | The PID file is missing — cannot determine tunnel state |

Dead tunnels are cleaned up automatically from the list on each `tunnel-list` invocation.

---

## `melisa tunnel-stop <container> [remote_port]`

Terminates one or all active tunnels for a given container.

```bash
# Stop the specific tunnel on port 3000
melisa tunnel-stop myapp 3000

# Stop ALL tunnels for this container (omit port)
melisa tunnel-stop myapp
```

The command:
1. Reads the `.pid` file for the matching tunnel key
2. Sends `SIGTERM` to the SSH process
3. Deletes both the `.pid` and `.meta` files from `~/.config/melisa/tunnels/`

If no matching tunnel is found, the command exits cleanly with an informational message — it is not an error.

---

## Tunnel State Files

All tunnel state is stored in `~/.config/melisa/tunnels/`. Files are named `<container>_<remote_port>.{pid,meta}`:

```
~/.config/melisa/tunnels/
├── myapp_8080.pid     ← PID of the background ssh -N -f process
├── myapp_8080.meta    ← Tunnel metadata
├── devlab_5432.pid
└── devlab_5432.meta
```

**`.meta` file format:**
```
container=myapp
container_ip=10.0.3.5
remote_port=8080
local_port=8080
server=root@192.168.1.100
started=2026-03-20 16:30:00
```

The state files persist across terminal sessions. A tunnel started in one terminal is visible and manageable from any other terminal.

---

## Common Patterns

**Development server access:**
```bash
# Start the container, deploy, then open a tunnel to the app
melisa --run myapp
melisa --up ./myapp/program.mel
melisa tunnel myapp 8080
# → open http://localhost:8080 in your browser
```

**Database access from local tooling:**
```bash
# Tunnel the container's PostgreSQL port to local
melisa tunnel myapp 5432
# → connect with: psql -h localhost -p 5432 -U postgres
```

**Multiple ports simultaneously:**
```bash
melisa tunnel myapp 8080         # Web server
melisa tunnel myapp 5432 15432   # DB (different local port to avoid conflict)
melisa tunnel-list               # Confirm both are active
```

**Cleanup:**
```bash
melisa tunnel-stop myapp         # Stop all myapp tunnels
melisa tunnel-list               # Confirm list is empty
```