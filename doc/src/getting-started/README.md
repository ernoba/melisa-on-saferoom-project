# Getting Started

This section walks you through installing MELISA from scratch — from compiling the server binary on your host machine to connecting the client from your workstation and provisioning your first container.

## Overview

MELISA operates on a **server–client architecture**:

```
┌────────────────────────────┐         ┌──────────────────────────┐
│    MELISA HOST (Server)    │         │  WORKSTATION (Client)    │
│                            │         │                          │
│  ┌──────────────────────┐  │   SSH   │  ┌────────────────────┐  │
│  │  melisa (Rust binary)│  │◄───────►│  │  melisa (Bash CLI) │  │
│  └──────────────────────┘  │         │  └────────────────────┘  │
│  ┌──────────────────────┐  │         └──────────────────────────┘
│  │   LXC Containers     │  │
│  │  ┌──┐  ┌──┐  ┌──┐    │  │
│  │  │C1│  │C2│  │C3│    │  │
│  │  └──┘  └──┘  └──┘    │  │
│  └──────────────────────┘  │
└────────────────────────────┘
```

## Checklist

Before you begin, verify you have the following:

| Requirement | Server | Client |
|-------------|--------|--------|
| Linux OS (Fedora, Ubuntu, Debian, or Arch) | ✅ Required | ❌ Not needed |
| Rust toolchain (`rustup`) | ✅ Required | ❌ Not needed |
| Physical/console terminal access | ✅ Required for setup | ❌ Not needed |
| Root / sudo privileges | ✅ Required | ⚠️ Needed once for installer |
| SSH client | ❌ Not needed | ✅ Required |
| Internet connection | ✅ Required | ✅ Required for first install |

> **Supported host distributions:** Fedora, RHEL, CentOS, Rocky Linux (`dnf`), Ubuntu (`apt-get`), Debian (`apt-get`), Arch Linux (`pacman`). Other distributions will fall back to `apt-get` defaults with a warning.

## The Three Steps

1. **[Install the Server](./server-installation.md)** — Compile the Rust binary, run `--setup`, and initialize the host environment.

2. **[Install the Client](./client-installation.md)** — Deploy the Bash client on your workstation and register your server profile.

3. **[Create Your First Container](./first-container.md)** — Search for a Linux distribution, provision a container, and enter it.

Estimated time: **15–30 minutes** depending on your internet speed and hardware.