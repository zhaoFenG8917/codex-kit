<div align="center">

# 🛠️ codex-kit

**The missing system toolkit for AI coding agents.**

[![Build](https://github.com/zhaoFenG8917/codex-kit/actions/workflows/release.yml/badge.svg)](https://github.com/zhaoFenG8917/codex-kit/actions/workflows/release.yml)
[![Release](https://img.shields.io/github/v/release/zhaoFenG8917/codex-kit)](https://github.com/zhaoFenG8917/codex-kit/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Single binary · Zero dependencies · Windows / Linux / macOS · JSON-native output

English | [简体中文](README.zh-CN.md)

</div>

---

## 🤔 Why codex-kit?

AI coding agents (Codex CLI, Claude Code, Cursor…) write code well but struggle with **operating systems** — especially on Windows:

| Pain | What agents hit in the wild |
|---|---|
| 💥 Shell fragmentation | `ls`, `tree`, `head`, `tail` are missing or behave differently across PowerShell / cmd / Git Bash |
| 🌀 Quoting hell | PowerShell vs bash quoting rules silently corrupt agent-generated commands |
| 🈶 Encoding chaos | Legacy GBK/GB18030 files (common on Chinese Windows) come back as mojibake |
| 🗿 Unstructured output | Agents burn tokens regex-parsing human-readable text |

### Design philosophy

- 🧩 **Don't reinvent the wheel** — content search belongs to `ripgrep`, file finding to `fd`, viewing to `bat`. codex-kit only fills the gaps they leave.
- 📦 **Single binary, zero dependencies** — one `.exe`, no runtime, no installer. Drop it on `PATH` and every agent can use it.
- 🤖 **Agent-first I/O** — every command supports `--format json`; in JSON mode errors are reported as `{"error": ...}` with a non-zero exit code and never pollute data streams.
- 🈶 **Encoding-safe** — BOM sniffing → strict UTF-8 → chardetng → GBK → GB18030 → Windows-1252 fallback. `replace` writes files back in their **original** encoding, BOM included.

## 📦 Installation

### From source (Cargo)

```bash
cargo install --git https://github.com/zhaoFenG8917/codex-kit
```

### Pre-built binaries

Grab the archive for your platform from [Releases](https://github.com/zhaoFenG8917/codex-kit/releases). No runtime required.

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/zhaoFenG8917/codex-kit/releases/latest/download/codex-kit-x86_64-pc-windows-msvc.zip" -OutFile codex-kit.zip
Expand-Archive codex-kit.zip -DestinationPath "$env:USERPROFILE\.local\bin"   # any dir on PATH works
```

```bash
# Linux / macOS
curl -L https://github.com/zhaoFenG8917/codex-kit/releases/latest/download/codex-kit-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv codex-kit /usr/local/bin/
```

## 🚀 Commands Quick Reference

Global flag: `--format plain|json` (default: `plain`).

| Command | Description |
|---|---|
| `codex-kit ls [path] [-a] [-l] [--limit N]` | 📄 List directory contents (--limit keeps JSON valid when truncating) |
| `codex-kit tree [path] [-d N] [--ignore glob]` | 🌲 Directory tree with depth limit and ignore globs |
| `codex-kit info <path>` | 🔍 File/dir metadata (type, size, timestamps, readonly) |
| `codex-kit stat` | 🖥️ Probe OS, arch, hostname, PATH, python/node/git versions |
| `codex-kit replace <file> <old> <new> [--dry-run] [--regex] [--no-backup]` | ✏️ Safe text replace — original encoding preserved, `.bak` backup by default |
| `codex-kit head <file> [-n N]` | ⬆️ First N lines, memory-safe on huge files |
| `codex-kit tail <file> [-n N]` | ⬇️ Last N lines, backward block-seek reading |
| `codex-kit wc <file>` | 🔢 Lines, words, chars, bytes |
| `codex-kit diff <f1> <f2>` | ➕➖ Unified diff (plain) / structured change list (json) |
| `codex-kit ps [name] [--limit N]` | 🧵 List processes (sorted by memory), optional name filter / top-N |
| `codex-kit kill <pid>` | 🔪 Kill a process by PID |
| `codex-kit port <port>` | 🔌 Check if a TCP port is listening, and which process owns it |
| `codex-kit read <file> --range 100:200` | 📖 Read a 1-based line range, streaming (no full-file load) |
| `codex-kit run [--timeout N] <cmd...>` | ⏱️ Run any command with timeout, exit-code passthrough, decoded output |
| `codex-kit start [--wait-port P] [--cwd d] [--log f] <cmd...>` | 🚀 Launch a detached background service — survives the session, logs to file, optionally waits for its port |
| `codex-kit which <cmd>` | 📍 Locate a command on PATH (+ version probe) |
| `codex-kit archive <out.zip\|out.tar.gz> <inputs...>` | 📦 Create zip / tar.gz archives |
| `codex-kit extract <archive> [-d dest]` | 📂 Extract zip / tar.gz (path-traversal safe) |
| `codex-kit write <file> [--encoding gbk] [--append]` | 💾 Write stdin to a file with an explicit encoding |
| `codex-kit hash <file> [-a md5\|sha256]` | #️⃣ Streaming file checksum |
| `codex-kit http <url> [-X M] [-H 'k: v'] [-d body] [--timeout N]` | 🌐 HTTP requests — any status is data (exit 0), only transport errors exit non-zero |
| `codex-kit exec <code>` / `-f <file.py>` | 🐍 Escape hatch: run code with the **system** Python |

## 🤖 Agent Integration

The real power move: teach your agent to route system operations through codex-kit.
Copy [AGENTS_TEMPLATE.md](AGENTS_TEMPLATE.md) into your project root as `AGENTS.md` (Codex) or `.cursorrules` (Cursor):

```markdown
# System Tool Routing (codex-kit)

This environment has `codex-kit`, `rg`, `fd` and `bat` installed.
Route system operations as follows:

1. Content search: `rg "pattern" [dir]`. File finding: `fd "pattern" [dir]`.
2. Directory & file operations via `codex-kit`:
    `ls` / `tree` / `info` / `stat` / `replace` / `head` / `tail` / `wc` / `diff` / `read` /
    `ps` / `kill` / `port` / `run` / `start` / `which` / `write` / `hash` / `archive` / `extract` / `http`.
3. Text files on Windows may be GBK-encoded — prefer `codex-kit head/tail/wc/replace`
   (automatic encoding detection) over raw shell reads.
4. For risky replacements run `codex-kit replace <file> <old> <new> --dry-run` first.
5. Escape hatch: `codex-kit exec "python_code"` runs the **system** Python.
   Never use a project's virtualenv interpreter for system operations.
6. Append `--format json` whenever you need to parse the output.
7. The command list above may lag the installed version. Run `codex-kit --help`
   (or `codex-kit <command> --help`) to discover the actual commands and flags —
   trust the `--help` output over this list.
```

## 🧩 Ecosystem

codex-kit complements the great Rust CLI tools instead of competing with them:

| Need | Tool | Why |
|---|---|---|
| Content search | [`ripgrep`](https://github.com/BurntSushi/ripgrep) | Blazing fast, respects `.gitignore` |
| File finding | [`fd`](https://github.com/sharkdp/fd) | Friendly syntax, fast traversal |
| File viewing | [`bat`](https://github.com/sharkdp/bat) | Syntax highlighting, paging |
| Everything else system-level | **codex-kit** | Uniform cross-platform behavior, JSON output, encoding-safe |

## 📄 License

[MIT](LICENSE) © 2026
