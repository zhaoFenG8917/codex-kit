<div align="center">

# 🛠️ codex-kit

**为 AI 编程 Agent 而生的系统操作工具集。**

[![Build](https://github.com/zhaoFenG8917/codex-kit/actions/workflows/release.yml/badge.svg)](https://github.com/zhaoFenG8917/codex-kit/actions/workflows/release.yml)
[![Release](https://img.shields.io/github/v/release/zhaoFenG8917/codex-kit)](https://github.com/zhaoFenG8917/codex-kit/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

单文件 · 零依赖 · Windows / Linux / macOS · 原生 JSON 输出

[English](README.md) | 简体中文

</div>

---

## 🤔 为什么是 codex-kit？

AI 编程 Agent（Codex CLI、Claude Code、Cursor……）写代码很在行，但操作起**操作系统**来却常常翻车——在 Windows 上尤其如此：

| 痛点 | Agent 的真实遭遇 |
|---|---|
| 💥 Shell 碎片化 | `ls`、`tree`、`head`、`tail` 在 PowerShell / cmd / Git Bash 里要么没有，要么行为不一致 |
| 🌀 引号地狱 | PowerShell 和 bash 的引号规则不同，Agent 生成的命令悄悄被改坏 |
| 🈶 编码乱码 | 中文 Windows 上大量 GBK/GB18030 遗留文件，读出来全是乱码 |
| 🗿 输出非结构化 | Agent 只能浪费 token 用正则解析人类可读的文本 |

### 设计哲学

- 🧩 **不重复造轮子** —— 内容搜索交给 `ripgrep`，文件查找交给 `fd`，文件查看交给 `bat`，codex-kit 只补全它们没覆盖的盲区。
- 📦 **单文件零依赖** —— 一个 `.exe`，不需要任何运行时和安装器。丢进 PATH，所有 Agent 都能用。
- 🤖 **为 Agent 设计的 I/O** —— 所有命令支持 `--format json`；JSON 模式下错误以 `{"error": ...}` 输出且退出码非零，绝不污染数据流。
- 🈶 **编码安全** —— BOM 嗅探 → 严格 UTF-8 → chardetng → GBK → GB18030 → Windows-1252 逐级降级。`replace` 写回文件时保持**原编码**（含 BOM），改 GBK 文件不会变成 UTF-8。

## 📦 安装

### 从源码安装（Cargo）

```bash
cargo install --git https://github.com/zhaoFenG8917/codex-kit
```

### 下载预编译二进制

到 [Releases](https://github.com/zhaoFenG8917/codex-kit/releases) 页面下载对应平台的压缩包，无需任何运行时。

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/zhaoFenG8917/codex-kit/releases/latest/download/codex-kit-x86_64-pc-windows-msvc.zip" -OutFile codex-kit.zip
Expand-Archive codex-kit.zip -DestinationPath "$env:USERPROFILE\.local\bin"   # 任何 PATH 中的目录均可
```

```bash
# Linux / macOS
curl -L https://github.com/zhaoFenG8917/codex-kit/releases/latest/download/codex-kit-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv codex-kit /usr/local/bin/
```

## 🚀 命令速查表

全局参数：`--format plain|json`（默认 `plain`）。

| 命令 | 说明 |
|---|---|
| `codex-kit ls [path] [-a] [-l]` | 📄 列出目录内容 |
| `codex-kit tree [path] [-d N] [--ignore glob]` | 🌲 目录树，支持深度限制和忽略规则 |
| `codex-kit info <path>` | 🔍 文件/目录元信息（类型、大小、时间戳、只读属性） |
| `codex-kit stat` | 🖥️ 探测系统环境（OS、架构、PATH、python/node/git 版本） |
| `codex-kit replace <file> <old> <new> [--dry-run] [--regex] [--no-backup]` | ✏️ 安全文本替换——保持原编码，默认生成 `.bak` 备份 |
| `codex-kit head <file> [-n N]` | ⬆️ 读取前 N 行，大文件不占内存 |
| `codex-kit tail <file> [-n N]` | ⬇️ 读取后 N 行，从文件尾部反向分块读取 |
| `codex-kit wc <file>` | 🔢 统计行数、词数、字符数、字节数 |
| `codex-kit diff <f1> <f2>` | ➕➖ 文件对比（plain 输出 unified diff，json 输出结构化变更） |
| `codex-kit ps [name]` | 🧵 进程列表，支持按名称模糊过滤 |
| `codex-kit kill <pid>` | 🔪 按 PID 结束进程 |
| `codex-kit port <port>` | 🔌 检查 TCP 端口是否在监听，以及被哪个进程占用 |
| `codex-kit read <file> --range 100:200` | 📖 按 1 起始的行号区间流式读取，不加载整个文件 |
| `codex-kit run [--timeout N] <cmd...>` | ⏱️ 带超时执行任意命令，透传退出码，自动解码输出 |
| `codex-kit which <cmd>` | 📍 在 PATH 中定位命令（含版本探测） |
| `codex-kit archive <out.zip\|out.tar.gz> <inputs...>` | 📦 创建 zip / tar.gz 压缩包 |
| `codex-kit extract <archive> [-d dest]` | 📂 解压 zip / tar.gz（防路径穿越） |
| `codex-kit write <file> [--encoding gbk] [--append]` | 💾 将 stdin 以指定编码写入文件 |
| `codex-kit hash <file> [-a md5\|sha256]` | #️⃣ 流式计算文件校验和 |
| `codex-kit exec <code>` / `-f <file.py>` | 🐍 兜底通道：调用**系统** Python 执行代码 |

## 🤖 接入 Agent

真正的杀手锏：让你的 Agent 自动把系统操作路由到 codex-kit。
把 [AGENTS_TEMPLATE.md](AGENTS_TEMPLATE.md) 复制到你项目的根目录，命名为 `AGENTS.md`（Codex）或 `.cursorrules`（Cursor）：

```markdown
# System Tool Routing (codex-kit)

This environment has `codex-kit`, `rg`, `fd` and `bat` installed.
Route system operations as follows:

1. Content search: `rg "pattern" [dir]`. File finding: `fd "pattern" [dir]`.
2. Directory & file operations via `codex-kit`:
    `ls` / `tree` / `info` / `stat` / `replace` / `head` / `tail` / `wc` / `diff` / `read` /
    `ps` / `kill` / `port` / `run` / `which` / `write` / `hash` / `archive` / `extract`.
3. Text files on Windows may be GBK-encoded — prefer `codex-kit head/tail/wc/replace`
   (automatic encoding detection) over raw shell reads.
4. For risky replacements run `codex-kit replace <file> <old> <new> --dry-run` first.
5. Escape hatch: `codex-kit exec "python_code"` runs the **system** Python.
   Never use a project's virtualenv interpreter for system operations.
6. Append `--format json` whenever you need to parse the output.
```

## 🧩 生态

codex-kit 与优秀的 Rust CLI 工具是互补关系，而不是竞争关系：

| 需求 | 工具 | 原因 |
|---|---|---|
| 内容搜索 | [`ripgrep`](https://github.com/BurntSushi/ripgrep) | 极速，自动遵守 `.gitignore` |
| 文件查找 | [`fd`](https://github.com/sharkdp/fd) | 语法友好，遍历快 |
| 文件查看 | [`bat`](https://github.com/sharkdp/bat) | 语法高亮，自动分页 |
| 其他系统级操作 | **codex-kit** | 跨平台行为统一、JSON 输出、编码安全 |

## 📄 许可证

[MIT](LICENSE) © 2026
