# Introduction

<div align="center">

```
 ███╗   ███╗███████╗██║     ██║███████╗███████╗
 ████╗ ████║██╔════╝██║     ██║██╔════╝██╔══██╗
 ██╔████╔██║█████╗  ██║     ██║███████╗███████║
 ██║╚██╔╝██║██╔══╝  ██║     ██║╚════██║██╔══██║
 ██║ ╚═╝ ██║███████╗███████╗██║███████║██║  ██║
 ╚═╝     ╚═╝╚══════╝╚══════╝╚═╝╚══════╝╚═╝  ╚═╝
     [ MANAGEMENT ENVIRONMENT LINUX SANDBOX ]
```

**v0.1.3** · Built with 🦀 Rust · MIT License

</div>

---

## What is MELISA?

**MELISA** (Management Environment Linux Sandbox) is a high-performance LXC container manager written in Rust, designed to solve one fundamental problem in software development: **host pollution**.

You know the feeling. You want to try a new language, test a risky library, or experiment with a system-level tool — but you're terrified of corrupting your pristine development machine. Or your team has the classic *"works on my machine"* problem. Or you're a teacher who needs to provision identical environments for 30 students in minutes.

MELISA solves all of this by turning a Linux host into a **Secure Orchestration Node** — a machine that carves out isolated, reproducible LXC containers on demand, manages users with fine-grained permissions, synchronizes collaborative projects through Git-backed pipelines, and deploys full container environments from a single manifest file — all controllable from any workstation in the world via a lightweight Bash client.

---

## The Three Pillars

MELISA is built around three components that work in concert:

### 🦀 The Server (Host Engine)

A compiled Rust binary (`melisa`) that runs on a Linux host. It acts as a **jail shell** — when users log in via SSH, they land directly inside the MELISA interactive prompt rather than a standard bash session. The engine manages LXC containers, enforces privilege separation, orchestrates Git-based project collaboration, and runs the Deployment Engine.

### 🐚 The Client (Remote Manager)

A modular Bash script (`melisa`) installed on any workstation. It wraps SSH to transparently forward commands to the remote MELISA host, allowing developers to manage containers, clone projects, sync code, execute scripts inside remote containers, and open SSH tunnels — all with a single, unified CLI.

### 📋 The Deployment Engine (`.mel` Manifests)

A TOML-based manifest system that describes an entire container environment in one file. Running `melisa --up` provisions the container, installs all dependencies, configures volumes and environment variables, and runs lifecycle hooks — fully automated, reproducible, and version-controllable alongside your code.

---

## Design Philosophy

| Principle | Implementation |
|-----------|----------------|
| **Zero Host Pollution** | All work happens inside LXC containers; the host OS remains clean |
| **Security by Presence** | System initialization requires physical terminal access — remote attackers cannot bootstrap the system |
| **Privilege Separation** | Two roles (Admin / Standard User) with surgically precise `sudoers` rules |
| **Async by Default** | The Rust engine is built on Tokio; no blocking I/O anywhere in the critical path |
| **Reproducibility** | Containers are provisioned from standardized LXC templates with deterministic post-install steps |
| **Git-Native Collaboration** | Projects are bare Git repositories; the push-to-deploy hook auto-syncs all members |
| **Declarative Deployment** | `.mel` manifests describe the full container stack; deploy with one command |

---

## What's Inside This Book?

This documentation is split into two parts:

**Part I — Technical Guide** is your reference manual. Every command, every flag, every configuration option is documented with examples, internal mechanics, and edge-case notes.

**Part II — The MELISA Chronicles** tells the same story through narrative. If you prefer to learn by following real characters through real workflows, start there and return here when you need specifics.