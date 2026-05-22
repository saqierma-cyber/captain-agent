<div align="center">

# 🛡️ 智能体队长 Captain Agent

### 新时代的电脑防护智能体

*你的 AI 智能体彻夜运行，智能体队长从不入睡。*

[English](./README.md) · [**简体中文**](./README.zh-CN.md)

![平台](https://img.shields.io/badge/平台-macOS-111111)
![Tauri](https://img.shields.io/badge/Tauri_2-Rust-CE412B)
![前端](https://img.shields.io/badge/React_19-TypeScript-3178C6)
![osquery](https://img.shields.io/badge/osquery-5.23.0-4E5D94)
![状态](https://img.shields.io/badge/状态-MVP-F5A623)
![许可证](https://img.shields.io/badge/许可证-MIT-2E7D32)
![云与遥测](https://img.shields.io/badge/云端_与_遥测-无-2E7D32)

</div>

---
<img width="2200" height="1440" alt="image" src="https://github.com/user-attachments/assets/b328b37d-f534-44ce-9847-b0a7516dfb3a" />

> ## 智能体的时代已经到来，它的队长也已就位。
>
> 你把 **Cursor**、**Claude Code** 以及一整支 AI 智能体队伍请进了电脑——你的文件、
> 你的命令行、你的网络，全部交到了它们手里——然后让它们在你熟睡时彻夜运行。它们
> 聪明、不知疲倦，而且**完全无人监管**。
>
> **智能体队长（Captain Agent）** 就是为此而生的新时代电脑防护智能体。它**守护你的
> 安全与隐私**，**保护你的财产安全**，确保**你的个人身份永不泄露**——它会记录每一个
> 智能体碰过的文件、跑过的命令、连过的网络对端，以及它试图埋下的每一处自启动后门。
>
> **我们的目标：做新时代的智能体队长，统帅这个时代的所有智能体。**

---

## ⚡ 为什么需要智能体队长

AI 编程智能体已经悄悄变成了"放手让它跑一晚"的工具。可当它跑完之后，你**没有任何
办法回溯**它到底干了什么：

- 它读过你的 `~/.ssh/id_rsa`、`~/.aws/credentials`、钥匙串吗？
- 它替你在本机执行了哪些 shell 命令？
- 它把流量发给了哪些 IP、哪些国家的服务器？
- 它有没有悄悄装一个 LaunchAgent，让自己在下次重启后依然存活？

现有工具各自只看到**一个切面**——Little Snitch 只看网络，活动监视器只看进程——而且
没有一个会按"**某一个 AI 智能体的整棵进程树**"来归集行为。它们连不起
"*读了一个机密文件*"→"*往外发了一个数据包*"这条线。

智能体队长，把这条线连起来。

## 🔍 它监控什么——每个智能体的四个维度

每一个被监控的程序，都会在它本机行为的**四个维度**上被全程跟踪。这四个维度，覆盖了
一个智能体伤害你的所有路径：

| 维度 | 捕获什么 | 为什么这能保护你 |
|---|---|---|
| 📁 **文件** | 对 SSH 私钥、`~/.aws/credentials`、GPG 密钥环、macOS 钥匙串、浏览器密码库、`.env` 文件的读写 | 这些文件**就是你的身份和你的凭据** |
| ⚙️ **进程** | 智能体派生的每一个子进程，连同完整命令行 | 一句 `curl … \| sh` 就能把你的电脑交给陌生人 |
| 🌐 **网络** | 连接元数据——远端地址、端口、域名、字节数（绝不解密 HTTPS） | 这是**你的数据离开电脑的出口** |
| 🔧 **持久化** | LaunchAgents、LaunchDaemons、Windows `Run` 注册表键、启动文件夹 | 这是威胁**在重启后存活、继续盯着你**的方式 |

## ⚖️ 判决如何作出——规则引擎

原始事件只是噪音。智能体队长内置 **48 条检测规则**，把噪音变成清晰的判决，覆盖
**三种规则类型**：

- **单事件规则**——一个动作就足够定性。*"读取 `~/.ssh/*`"* → **高危**。
- **关联规则**——时间窗口内的一段危险**序列**。*"读了 SSH 私钥，30 秒内又往网络推送
  数据"* → **严重**。
- **派生指标规则**——一种可疑的**速率**。*"出站流量超过 10 MB/分钟"* → 标记告警。

| 规则包 | 数量 | 示例 |
|---|---|---|
| 🔑 凭据类 | 13 | SSH 私钥、AWS / GCP 凭据、kubeconfig、GitHub token、浏览器密码库、`.env` |
| 💻 命令类 | 16 | `curl … \| sh`、反弹 shell（`bash -i`、`/dev/tcp/`）、`chmod -R 777`、`rm -rf /` |
| 🌐 网络类 | 7 | 已知恶意域名、异常的大流量外发 |
| 🪝 持久化类 | 5 | LaunchAgents / LaunchDaemons、Windows 自启动键、启动文件夹 |
| 🔗 关联类 | 3 | `credential-exfil`（凭据外泄）、`code-fetch-exec`（下载即执行）、`launchctl-injection` |
| 📈 指标类 | 4 | 持续高外发流量、进程派生风暴 |

每一次命中都会生成一条 **Finding（发现）**，带有严重级别（`info` → `critical`）和
一条由你掌控的生命周期：**待处理 → 已确认 / 已忽略 / 已加白**。`critical` 级别的发现
会在发生的瞬间**弹出系统原生通知**。你也可以直接在界面里用 YAML **编写自己的规则**
——它们会热重载，无需重启。

## 🏗️ 它如何工作

```
            ┌────────────────────────────────────────────────┐
            │  智能体队长  ·  Tauri 应用（运行在你的用户态）  │
            │  仪表盘 · 发现 · 时间线 · 监控目标 ·            │
            │  规则 · 设置             （5 种界面语言）       │
            └────────────────────────────────────────────────┘
                        ▲ 实时事件 / 发现        │ 规则增删改查
                        │                        ▼
            ┌────────────────────────────────────────────────┐
            │  Rust 核心：事件总线 → 规则引擎 →                │
            │  SQLite 存储（WAL）→ 系统通知                    │
            └────────────────────────────────────────────────┘
                        ▲ Unix 域套接字上的 JSON-Lines 协议
            ┌────────────────────────────────────────────────┐
            │  captain-helper  ·  LaunchDaemon（以 root 运行） │
            │  托管 osqueryd 子进程，转发其事件流              │
            └────────────────────────────────────────────────┘
                        ▲ stdout JSON 事件日志
            ┌────────────────────────────────────────────────┐
            │  osqueryd 5.23.0  ·  Apple Endpoint Security    │
            │  进程 / 文件 / 套接字 / DNS 事件（自带 PID）     │
            └────────────────────────────────────────────────┘
```

繁重的、与操作系统强相关的事件采集，交给了 **[osquery]**——它在 macOS 上使用 Apple 的
**Endpoint Security** 框架（在 Windows 上使用 ETW），所以事件抵达时**已经带好了 PID
归属**。智能体队长不 fork osquery，而是**嵌入官方签名版本**并读取它的事件流。

由于 Endpoint Security 需要 **root 权限**，一个小巧的 `captain-helper` 守护进程会作为
macOS **LaunchDaemon** 以 root 身份运行，托管 `osqueryd`（崩溃后按指数退避自动重启），
并通过 Unix 套接字把事件转发给用户态的 Tauri 应用。这与 Little Snitch、LuLu、
CrowdStrike Falcon 采用的是同一套架构。

[osquery]: https://github.com/osquery/osquery

## 📦 项目状态

**MVP 阶段——macOS 优先。** 端到端链路已完整打通：osqueryd → helper → 事件总线 →
规则引擎 → SQLite → 全部六个界面视图，并带有系统通知、HTML 报告导出，以及一套五语言
界面（中文 · 英文 · 日文 · 韩文 · 阿拉伯文）。

| 能力 | macOS | Windows |
|---|---|---|
| 进程 / 派生事件 | ✅ | 🛣️ 路线图中 |
| 文件写入 / 持久化事件 | ✅ | 🛣️ 路线图中 |
| 文件**读取**事件 | ⚠️ 部分支持——见《已知限制》 | 🛣️ 路线图中 |
| 网络与 DNS 元数据 | ✅ | 🛣️ 路线图中 |
| 规则引擎 · 发现 · 仪表盘 | ✅ | ✅（界面本身跨平台） |

## 🚀 快速开始（开发模式）

**前置条件：** macOS 13+、Rust 稳定版工具链、Node.js + `pnpm`，以及 Xcode 命令行工具
（`xcode-select --install`）。

```sh
# 1. 安装 JS 依赖
pnpm install

# 2. 拉取锁定版本的、带签名的 osquery 5.23.0（约 105 MB 的 .app 包）
./scripts/fetch-osqueryd.sh

# 3. 把 root helper 安装为 LaunchDaemon（会提示输入你的密码）
sudo ./scripts/install-helper.sh

# 4. 以开发模式运行桌面应用
pnpm tauri dev
```

如果哪里看起来不对劲，`./scripts/captain-diagnose.sh` 会检查 helper、套接字和你的
TCC 授权状态。

## 🔐 macOS 权限

`osqueryd` 使用 Endpoint Security 框架，而 macOS 用一道权限把它拦在门外。首次运行时，
请在 **系统设置 → 隐私与安全性 → 完全磁盘访问权限** 里，把权限授予 **`osqueryd`**
（而**不是**智能体队长本身）。

> **注意：** 对于一个 Endpoint Security 客户端，macOS 会把这次授权记录在内部服务
> `kTCCServiceEndpointSecurityClient` 名下——所以即便开关存储在别处，`osqueryd` 仍会
> 出现在「完全磁盘访问权限」列表里。没有这次授权，事件表会一直是空的。

## ⚠️ 已知限制（我们相信诚实的软件）

一款你无法相信它对你诚实的安全工具，毫无价值。所以：

- **macOS 14–16 上的文件读取检测是部分支持的。** osquery 5.23 的 Endpoint Security
  FIM 在较新的 macOS 上不能可靠地发出文件 *open* 事件，因此部分 `action: read` 规则
  可能不会触发。写入 / 创建 / 删除检测是可靠的。带 PID 归属的读取事件
  （`es_process_file_events`）是后续的解决方向。
- **代码签名尚未完成。** 在应用与 helper 用 Apple Developer ID 签名之前，每一次本地
  重新编译都会改变二进制的代码哈希，**可能导致 macOS TCC 授权失效**——届时你需要重新
  开关一次「完全磁盘访问权限」。
- **Windows 支持在路线图中**，不在本次 MVP 内。采集层为此预留了设计（osquery 在
  Windows 上使用 ETW），界面本身已经跨平台。
- **设计上只审计、不拦截。** 智能体队长**只记录、只告警，绝不阻断**。它是一台行车
  记录仪，不是一道防火墙。

## 🗂️ 仓库结构

```
captain-agent/
├── captain-common/         共享类型：Event、Finding、Rule、Target、IPC 消息
├── captain-helper/         root LaunchDaemon——托管 osqueryd，UDS 事件服务端
│   └── src/osquery/        子进程托管 · 配置生成 · 事件归一化
├── src-tauri/              Tauri 应用——Rust 核心
│   └── src/
│       ├── bus.rs          ③ 事件总线（tokio broadcast）
│       ├── helper_client.rs   UDS 客户端——抽取 helper 的事件流
│       ├── target/         ① 监控目标管理器——决定监控哪些应用与 PID 树
│       ├── rule/           ④ 规则引擎——单事件 / 关联 / 派生指标
│       │   └── builtin/    48 条内置检测规则（6 个 YAML 规则包）
│       ├── store/          ⑤ SQLite 存储 + 异步批量写入
│       ├── api/            ⑥ Tauri 命令 + 实时事件推送
│       └── notify.rs       ⑧ 系统原生通知
├── src/                    ⑦ React 界面——6 个视图 + 5 语言 i18n
├── scripts/                fetch-osqueryd · install/uninstall-helper · diagnose
└── tests/scenarios/        模拟智能体越界行为的 shell 脚本
```

## 🛣️ 路线图

- **Windows** —— 把 `osqueryd` 注册为 `LocalSystem` 服务，发布 MSI 安装包。
- **MITRE ATT&CK 标签** —— 给每条发现打上技战术编号（`T1003`、`T1547`……）。
- **Sigma 规则导入** —— 继承社区 SIEM 检测规则库。
- **可选的 HTTPS 明文模式** —— 一个高级的、需主动开启的选项，解密流量正文以做更深检查。
- **二进制信誉** —— 对派生出的可执行文件做哈希比对，核对白名单 / 黑名单。

## 🤫 我们的隐私承诺

- **不上云。** 每一条规则、每一个事件、每一条发现，都只留在你的电脑上。
- **无遥测。** 智能体队长**不会**把任何关于你的信息发往任何地方。
- **不解密 HTTPS**（MVP 阶段）——我们只采集连接的*元数据*，绝不触碰你流量的内容。

一款守护你隐私的工具，绝不能先去侵犯它。

## 📜 许可证与致谢

- 智能体队长自身源码以 **MIT 许可证**发布——见 [`LICENSE`](./LICENSE)。
- **[osquery]**（Apache-2.0）—— 作为跨平台采集层被嵌入。
- 检测模式参考了公开知识：Wazuh 规则集、SigmaHQ、MITRE ATT&CK，以及 Objective-See
  对 macOS 持久化机制的研究。我们不 fork 它们的任何代码——只借鉴公开已知的指标字符串。

---

<div align="center">

**智能体队长 · Captain Agent**

*智能体的时代，总要有人站岗放哨。来做那个队长。*

</div>
