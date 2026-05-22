<div align="center">

# 🛡️ Captain Agent

### 智能体队长 — the new-era guardian agent for your computer

*Your AI agents run all night. Captain Agent never sleeps.*

[**English**](./README.md) · [简体中文](./README.zh-CN.md)

![Platform](https://img.shields.io/badge/platform-macOS-111111)
![Tauri](https://img.shields.io/badge/Tauri_2-Rust-CE412B)
![Frontend](https://img.shields.io/badge/React_19-TypeScript-3178C6)
![osquery](https://img.shields.io/badge/osquery-5.23.0-4E5D94)
![Status](https://img.shields.io/badge/status-MVP-F5A623)
![License](https://img.shields.io/badge/license-MIT-2E7D32)
![Cloud & telemetry](https://img.shields.io/badge/cloud_&_telemetry-none-2E7D32)

</div>

---

> ## The age of autonomous agents has arrived. So has its captain.
>
> You hand **Cursor**, **Claude Code**, and a whole fleet of AI agents the keys to
> your machine — your files, your shell, your network — and you let them run while
> you sleep. They are brilliant. They are tireless. And they are **completely
> unsupervised**.
>
> **Captain Agent** is the new-era computer-defense agent that stands watch over
> them. It **guards your security and privacy**, **shields your property and
> financial safety**, and makes sure **your personal identity is never leaked** —
> by recording every file an agent touches, every command it spawns, every network
> peer it talks to, and every persistence hook it tries to plant.
>
> **Our mission: to become the Captain that commands the agents of the new era.**

---

## ⚡ Why Captain Agent exists

AI coding agents have quietly become "let it run overnight" tools. But after the
run, you have **no way to look back** at what they actually did:

- Did it read your `~/.ssh/id_rsa`, your `~/.aws/credentials`, your keychain?
- What shell commands did it execute on your behalf?
- Which IP addresses — in which countries — did it send traffic to?
- Did it quietly install a LaunchAgent so it survives the next reboot?

Existing tools each see only **one slice** of the picture — Little Snitch watches
only the network, Activity Monitor watches only processes — and none of them group
behavior by **"this one AI agent's process tree."** They can't connect the dots
between *"read a secret file"* → *"sent a packet out."*

Captain Agent connects those dots.

## 🔍 What it watches — four dimensions of every agent

Every monitored program is followed across **four dimensions** of its behavior on
your machine. Together they cover how an agent can hurt you:

| Dimension | What it captures | Why it protects you |
|---|---|---|
| 📁 **Files** | Reads & writes of SSH keys, `~/.aws/credentials`, GPG keyrings, macOS Keychains, browser password databases, `.env` files | These files **are your identity and your credentials** |
| ⚙️ **Processes** | Every sub-process an agent spawns, with full command line | One `curl … \| sh` can hand your machine to a stranger |
| 🌐 **Network** | Connection metadata — remote address, port, domain, byte counts (no HTTPS decryption, ever) | This is where **your data leaves the building** |
| 🔧 **Persistence** | LaunchAgents, LaunchDaemons, Windows `Run` keys, startup folders | This is how a threat **survives a reboot and keeps watching** |

## ⚖️ How the verdict is reached — the rule engine

Raw events are just noise. Captain Agent ships **48 built-in detection rules** that
turn noise into a clear verdict, across **three rule types**:

- **Single-event rules** — one action is enough. *"Read `~/.ssh/*`"* → **high
  severity**.
- **Correlation rules** — a dangerous *sequence* inside a time window. *"Read an
  SSH key, then push bytes onto the network within 30 s"* → **critical**.
- **Metric rules** — a suspicious *rate*. *"Outbound traffic above 10 MB/min"* →
  flagged.

| Rule pack | Count | Examples |
|---|---|---|
| 🔑 Credentials | 13 | SSH keys, AWS / GCP creds, kubeconfig, GitHub tokens, browser password DBs, `.env` |
| 💻 Commands | 16 | `curl … \| sh`, reverse shells (`bash -i`, `/dev/tcp/`), `chmod -R 777`, `rm -rf /` |
| 🌐 Network | 7 | Known-malicious domains, anomalous large outbound transfers |
| 🪝 Persistence | 5 | LaunchAgents / LaunchDaemons, Windows autorun keys, startup folders |
| 🔗 Correlations | 3 | `credential-exfil`, `code-fetch-exec`, `launchctl-injection` |
| 📈 Metrics | 4 | Sustained high egress, process-spawn storms |

Every match becomes a **Finding** with a severity (`info` → `critical`) and a
lifecycle you control: **open → confirmed / dismissed / whitelisted**. `critical`
findings fire a **native OS notification** the moment they happen. You can also
**write your own rules** in YAML straight from the UI — they hot-reload with no
restart.

## 🏗️ How it works

```
            ┌────────────────────────────────────────────────┐
            │  Captain Agent  ·  Tauri app  (your user mode)  │
            │  Dashboard · Findings · Timeline · Targets ·    │
            │  Rules · Settings        (5 interface languages)│
            └────────────────────────────────────────────────┘
                        ▲ live events / findings   │ rule CRUD
                        │                          ▼
            ┌────────────────────────────────────────────────┐
            │  Rust core: event bus → rule engine →           │
            │  SQLite store (WAL) → OS notifications          │
            └────────────────────────────────────────────────┘
                        ▲ JSON-Lines over a Unix domain socket
            ┌────────────────────────────────────────────────┐
            │  captain-helper  ·  LaunchDaemon (runs as root) │
            │  supervises osqueryd, forwards its event stream │
            └────────────────────────────────────────────────┘
                        ▲ stdout JSON event log
            ┌────────────────────────────────────────────────┐
            │  osqueryd 5.23.0  ·  Apple Endpoint Security    │
            │  process / file / socket / DNS events (w/ PID)  │
            └────────────────────────────────────────────────┘
```

The heavy, OS-specific event collection is delegated to **[osquery]** — it uses
Apple's **Endpoint Security** framework on macOS (and ETW on Windows), so events
arrive already attributed to a PID. Captain Agent does not fork osquery; it
**embeds the official signed build** and reads its event stream.

Because Endpoint Security requires **root**, a small `captain-helper` daemon runs
as a macOS **LaunchDaemon**, supervises `osqueryd` (auto-restart with exponential
backoff), and forwards events to the user-mode Tauri app over a Unix socket. This
is the same architecture used by Little Snitch, LuLu, and CrowdStrike Falcon.

[osquery]: https://github.com/osquery/osquery

## 📦 Project status

**MVP — macOS first.** The end-to-end pipeline is complete: osqueryd → helper →
event bus → rule engine → SQLite → all six UI views, with notifications, HTML
report export, and a 5-language interface (Chinese · English · Japanese · Korean ·
Arabic).

| Capability | macOS | Windows |
|---|---|---|
| Process / spawn events | ✅ | 🛣️ roadmap |
| File write / persistence events | ✅ | 🛣️ roadmap |
| File **read** events | ⚠️ partial — see *Limitations* | 🛣️ roadmap |
| Network & DNS metadata | ✅ | 🛣️ roadmap |
| Rule engine · Findings · Dashboard | ✅ | ✅ (UI is cross-platform) |

## 🚀 Quick start (development)

**Prerequisites:** macOS 13+, the Rust stable toolchain, Node.js + `pnpm`, and
Xcode Command Line Tools (`xcode-select --install`).

```sh
# 1. Install JS dependencies
pnpm install

# 2. Fetch the pinned, signed osquery 5.23.0 build (~105 MB .app bundle)
./scripts/fetch-osqueryd.sh

# 3. Install the root helper as a LaunchDaemon (prompts for your password)
sudo ./scripts/install-helper.sh

# 4. Run the desktop app in dev mode
pnpm tauri dev
```

If something looks wrong, `./scripts/captain-diagnose.sh` checks the helper,
the socket, and your TCC permission state.

## 🔐 macOS permissions

`osqueryd` uses the Endpoint Security framework, which macOS gates behind a
permission. On first run, grant **Full Disk Access** to **`osqueryd`** (not to
Captain Agent itself) under **System Settings → Privacy & Security → Full Disk
Access**.

> **Note:** for an Endpoint Security client, macOS records this grant under the
> internal service `kTCCServiceEndpointSecurityClient` — so `osqueryd` may appear
> in the **Full Disk Access** list even though the toggle is stored separately.
> Without the grant, the event tables stay empty.

## ⚠️ Known limitations (we believe in honest software)

A security tool you can't trust to be honest with you is worthless. So:

- **File-read detection on macOS 14–16 is partial.** osquery 5.23's Endpoint
  Security FIM does not reliably emit file-*open* events on recent macOS, so some
  `action: read` rules may not fire. Write / create / delete detection is solid.
  PID-attributed reads via `es_process_file_events` are the path forward.
- **Code signing is pending.** Until the app and helper are signed with an Apple
  Developer ID, every local rebuild changes the binary's code hash and **may
  invalidate the macOS TCC grant** — you would need to re-toggle Full Disk Access.
- **Windows support is on the roadmap**, not in this MVP. The collection layer is
  designed for it (osquery uses ETW on Windows); the UI is already cross-platform.
- **Audit-only by design.** Captain Agent **records and alerts — it never blocks**.
  It is a flight recorder, not a firewall.

## 🗂️ Repository layout

```
captain-agent/
├── captain-common/         Shared types: Event, Finding, Rule, Target, IPC messages
├── captain-helper/         Root LaunchDaemon — supervises osqueryd, UDS event server
│   └── src/osquery/        Supervisor · config generator · normalizer
├── src-tauri/              Tauri app — the Rust core
│   └── src/
│       ├── bus.rs          ③ Event bus (tokio broadcast)
│       ├── helper_client.rs   UDS client — drains the helper's event stream
│       ├── target/         ① Target Manager — which apps & PID trees to watch
│       ├── rule/           ④ Rule engine — single / correlation / metric
│       │   └── builtin/    48 built-in detection rules (6 YAML packs)
│       ├── store/          ⑤ SQLite store + async batch writer
│       ├── api/            ⑥ Tauri commands + live event push
│       └── notify.rs       ⑧ Native OS notifications
├── src/                    ⑦ React UI — 6 views + 5-language i18n
├── scripts/                fetch-osqueryd · install/uninstall-helper · diagnose
└── tests/scenarios/        Shell scripts that simulate agent misbehavior
```

## 🛣️ Roadmap

- **Windows** — register `osqueryd` as a `LocalSystem` service, ship an MSI.
- **MITRE ATT&CK tags** — label each finding with a technique ID (`T1003`, `T1547`…).
- **Sigma rule import** — inherit the community SIEM detection corpus.
- **Optional HTTPS plaintext mode** — an advanced opt-in that decrypts traffic
  bodies for deeper inspection.
- **Binary reputation** — hash-check spawned executables against allow/deny lists.

## 🤫 Our privacy promise

- **No cloud.** Every rule, every event, every finding stays on your machine.
- **No telemetry.** Captain Agent sends *nothing* about you anywhere.
- **No HTTPS decryption** in the MVP — we capture connection *metadata* only, never
  the content of your traffic.

A tool that guards your privacy must not violate it first.

## 📜 License & credits

- Captain Agent's own source is released under the **MIT License** — see
  [`LICENSE`](./LICENSE).
- **[osquery]** (Apache-2.0) — embedded as the cross-platform collection layer.
- Detection patterns are informed by open knowledge: the Wazuh ruleset, SigmaHQ,
  MITRE ATT&CK, and Objective-See's macOS persistence research. No code is forked
  from them — only publicly known indicator strings.

---

<div align="center">

**Captain Agent · 智能体队长**

*In the age of autonomous agents, someone has to stand watch. Be the Captain.*

</div>
