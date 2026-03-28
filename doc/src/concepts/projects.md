# Projects & Collaboration

MELISA's project system turns a Linux server into a **Git-backed collaborative development hub**. Projects are bare Git repositories stored on the host, with each team member getting their own working clone in their home directory.

---

## The Architecture

```
/opt/melisa/projects/
└── myapp/          ← Master Bare Repository (source of truth)
    ├── HEAD
    ├── config
    ├── hooks/
    │   └── post-receive   ← Auto-sync hook (triggers on push)
    ├── objects/
    └── refs/

/home/
├── alice/
│   └── myapp/      ← Alice's working clone (regular repo)
├── bob/
│   └── myapp/      ← Bob's working clone (regular repo)
└── carol/
    └── myapp/      ← Carol's working clone (regular repo)
```

The master repository at `/opt/melisa/projects/myapp/` is a **bare repository** — it has no working tree, only Git internals. It acts as the shared remote that all users push to and pull from.

---

## The Post-Receive Hook

When any team member pushes code to the master repository, a `post-receive` hook fires automatically:

```bash
#!/bin/bash
sudo melisa --update-all myapp
```

This forces the server to immediately propagate the latest code from the master repository to **every user's working directory** that has been invited to the project. No manual `git pull` needed — the server does it for you.

---

## Project Lifecycle

### Creating a Project (Admin only)

```
melisa@host:~> melisa --new_project myapp
```

Internally:
1. Creates `/opt/melisa/projects/myapp/`
2. Initializes a bare repository with `git init --bare --shared=group`
3. Sets ownership to `root:melisa` with SetGID bit (`chmod 2775`) so all files created inside inherit the group
4. Registers the path as a global git safe directory
5. Writes and makes executable the `post-receive` hook

### Inviting Team Members (Admin only)

```
melisa@host:~> melisa --invite myapp alice bob carol
```

For each user, MELISA:
1. Removes any existing (potentially corrupted) copy of the project from their home
2. Registers the master path as a git safe directory for that user's context
3. Runs `git clone` from the master repo into `/home/<username>/myapp/`
4. Sets group ownership and permissions on the cloned directory
5. Configures the clone's remote to point to the master

### Listing Your Projects

```
melisa@host:~> melisa --projects
```

This scans the user's home directory for directories that correspond to known master projects.

### Removing a Member (Admin only)

```
melisa@host:~> melisa --out myapp alice
```

Removes Alice's working clone (`rm -rf /home/alice/myapp`) and revokes her access. The master repository and other members' clones are unaffected.

---

## Synchronization Mechanics

### Admin Force-Pull

An administrator can pull code from **any user's workspace** into the master:

```
melisa@host:~> melisa --pull alice myapp
```

This is useful for code review, rescuing work from a departing team member, or resolving conflicts.

### Per-User Update

Updates a specific user's working copy from master:

```
melisa@host:~> melisa --update myapp
```

With `--force`, it performs a hard reset, discarding any local uncommitted changes:

```
melisa@host:~> melisa --update myapp --force
```

You can also target another user (admin only):

```
melisa@host:~> melisa --update alice myapp --force
```

### Mass Update

Propagates the master state to **all invited members** simultaneously:

```
melisa@host:~> melisa --update-all myapp
```

This is called automatically by the `post-receive` hook. You can also invoke it manually.

---

## Client-Side Synchronization

From your local workstation (MELISA client), the workflow is:

```bash
# Clone the project to your local machine
melisa clone myapp

# Make changes locally, then push everything to the server
cd myapp
# ... edit files ...
melisa sync

# Pull the latest data from your server-side workspace
melisa get myapp
```

### `melisa sync` in Detail

`sync` is an intelligent push command:

1. Identifies the current project by scanning the local path registry (`~/.config/melisa/registry`)
2. Stages all changes: `git add .`
3. Commits with an automatic timestamp message: `melisa-sync: 2026-03-20 16:30`
4. Force-pushes to the master repository on the server
5. The `post-receive` hook on the server fires automatically, running `melisa --update-all <project>` to propagate the latest commit to all member workspaces — no explicit client-side SSH call is needed for this step
6. Syncs any `.env` files via `rsync -azR` to the user's server-side workspace (because `.env` files are typically `.gitignore`d but still needed on the server)

### `melisa get` in Detail

Retrieves the latest data from your **server-side working copy** (not the master) to your local machine via Rsync:

- **Default mode**: `--ignore-existing` — only downloads files that don't exist locally
- **Force mode** (`--force`): Full overwrite, replaces all local files with server versions

---

## Deleting a Project (Admin only)

```
melisa@host:~> melisa --delete_project myapp
```

This permanently deletes:
1. The master repository at `/opt/melisa/projects/myapp/`
2. Every team member's working clone at `/home/<username>/myapp/`

> **This operation is irreversible. All committed history is lost.**

---

## Git Security Configuration

MELISA applies two important Git configurations system-wide:

```bash
git config --system --add safe.directory /opt/melisa/projects/myapp
git config --system --add safe.directory '*'
```

This prevents Git's "dubious ownership" fatal error, which triggers when a repository is accessed by a user who isn't the directory owner — a common scenario in shared environments.