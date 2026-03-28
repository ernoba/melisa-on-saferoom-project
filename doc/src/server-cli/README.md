# Server CLI Reference

The MELISA server CLI is an **interactive shell environment** that replaces the standard bash login shell for all MELISA users. It is also invokable non-interactively via SSH for scripted operations.

---

## Invocation Modes

### Interactive Mode

When you log in via SSH or run `melisa` directly, you enter the interactive REPL:

```
melisa@hostname:~> _
```

The prompt format is: `melisa@<username>:<current_path>> `

The REPL supports:
- **Command history** — Navigate with ↑/↓ arrows; persisted to `~/.melisa_history`
- **History-based autocomplete** — Suggestions appear as greyed-out text as you type
- **File path completion** — Tab-completion for paths when using `cd` or typing a `/`
- **Bracket validation** — Matching bracket highlighting as you type

### Non-Interactive (SSH passthrough)

The client uses this mode internally to forward commands:

```bash
# Called as: melisa -c "command args"
ssh root@server "melisa --list"
ssh root@server "melisa --send mybox apt update"
```

`main.rs` detects the `-c` flag pattern and routes the argument string directly to `execute_command` without starting the REPL.

---

## The `--audit` Flag

The `--audit` flag is a **global debugging modifier** that can be appended to any MELISA command in any position:

```bash
melisa --create mybox ubu-jammy-x64 --audit
melisa --audit --delete mybox
melisa --stop mybox --audit
```

When `--audit` is present:

1. **The spinner is hidden** — No progress bar is shown, so raw subprocess output flows directly to the terminal without being obscured.
2. **Raw subprocess output is inherited** — All output from child processes (`lxc-*`, `git`, `apt`, `userdel`, etc.) streams directly to your terminal instead of being suppressed.
3. **Debug messages are shown** — Internal diagnostics that are normally hidden (e.g., fallback protocol decisions in `--search`) are printed inline.

`--audit` is useful when a command hangs or fails and the spinner is hiding the actual error. It turns the polished UI into a transparent debug window without requiring log files.

---

## Command Groups

### General Commands
Available to all users:

| Command | Description |
|---------|-------------|
| [`--help`, `-h`](./container-lifecycle.md) | Display the help manual (role-aware: admins see extra sections) |
| [`--version`](./setup.md) | Print MELISA version and author information |
| [`--list`](./container-lifecycle.md) | List all LXC containers |
| [`--active`](./container-lifecycle.md) | List only running containers |
| [`--run <n>`](./container-lifecycle.md) | Start a container |
| [`--stop <n>`](./container-lifecycle.md) | Stop a container |
| [`--use <n>`](./container-interaction.md) | Attach an interactive shell to a container |
| [`--send <n> <cmd>`](./container-interaction.md) | Execute a non-interactive command inside a container |
| [`--info <n>`](./container-interaction.md) | Display container metadata |
| [`--ip <n>`](./container-interaction.md) | Get the internal IP address of a container |
| [`--upload <n> <dest>`](./container-interaction.md) | Upload a tarball stream into a container |
| [`--mel-info <file.mel>`](./deployment-engine.md) | Display parsed info for a `.mel` manifest |
| [`--projects`](./project-management.md) | List projects in your workspace |
| [`--update <project> [--force]`](./project-management.md) | Sync your working copy from the master repo |
| `cd <path>` | Change directory within the MELISA shell session |
| `exit`, `quit` | Terminate the MELISA session |

### Administration Commands
Available to **Administrators** only:

| Command | Description |
|---------|-------------|
| [`--setup`](./setup.md) | Initialize the host environment |
| [`--clear`](./setup.md) | Purge the command history |
| [`--clean`](./user-management.md) | Remove orphaned sudoers files |
| [`--search <keyword>`](./container-lifecycle.md) | Search available LXC distributions |
| [`--create <n> <code>`](./container-lifecycle.md) | Provision a new container |
| [`--delete <n>`](./container-lifecycle.md) | Destroy a container |
| [`--share <n> <host> <cont>`](./container-interaction.md) | Mount a host directory into a container |
| [`--reshare <n> <host> <cont>`](./container-interaction.md) | Unmount a host directory from a container |
| [`--up <file.mel>`](./deployment-engine.md) | Deploy a project from a `.mel` manifest |
| [`--down <file.mel>`](./deployment-engine.md) | Stop a deployment defined in a `.mel` manifest |
| [`--add <user>`](./user-management.md) | Create a new MELISA user |
| [`--remove <user>`](./user-management.md) | Delete a MELISA user |
| [`--upgrade <user>`](./user-management.md) | Promote a user to Admin |
| [`--passwd <user>`](./user-management.md) | Change a user's password |
| [`--user`](./user-management.md) | List all MELISA users |
| [`--new_project <n>`](./project-management.md) | Create a new master project repository |
| [`--delete_project <n>`](./project-management.md) | Delete a project and all member copies |
| [`--invite <proj> <users...>`](./project-management.md) | Invite users to a project |
| [`--out <proj> <users...>`](./project-management.md) | Remove users from a project |
| [`--pull <user> <proj>`](./project-management.md) | Pull code from a user's workspace to master |
| [`--update-all <proj>`](./project-management.md) | Push master to all member workspaces |

---

## Built-in Shell Commands

### `cd`
Changes the current directory within the MELISA session. Supports `~` as a shorthand for the user's home directory:

```
melisa@host:~> cd /opt
melisa@host:/opt> cd ~
melisa@host:~>
```

### `exit` / `quit`
Gracefully terminates the session with a farewell message.

### Bash Passthrough
Any unrecognized command is transparently dispatched to the system bash shell:

```
melisa@host:~> ls -la
melisa@host:~> cargo build --release
melisa@host:~> python3 my_script.py
```

The passthrough inherits the user's `HOME`, `USER`, `PATH` (extended with `~/.cargo/bin`), and Rustup/Cargo environment variables, making it suitable for development work.