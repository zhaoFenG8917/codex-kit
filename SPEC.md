# codex-kit 项目规格说明书 (SPEC.md)

## 1. 项目概述
`codex-kit` 是一个专为 AI Coding Agent（如 Codex CLI）设计的跨平台系统操作 CLI 工具集。
它旨在解决 Agent 在 Windows 环境下执行系统级 bash 命令（如 `ls`, `tree`, `cat` 等）时的兼容性差、速度慢、环境冲突等痛点。

本项目采用 Rust 编写，编译为单一零依赖二进制文件，通过加入系统 PATH 即可被 Agent 自动发现和调用。

## 2. 核心设计原则
1. **纯粹的 CLI 工具，非 Skill**：不依赖任何 Agent 框架的 Skill 机制，完全通过标准的 stdin/stdout/stderr 和退出码与 Agent 交互。
2. **只做系统操作，隔离项目环境**：绝不触碰项目的虚拟环境、依赖管理或构建流程，避免环境冲突。
3. **不重复造轮子**：内容搜索使用 `ripgrep (rg)`，文件查找使用 `fd`，文件查看使用 `bat`。`codex-kit` 只补全这些工具未覆盖的系统操作盲区。
4. **零依赖部署**：Release 构建必须是单个 `.exe`（Windows）或单文件（Linux/macOS），无需安装任何运行时。
5. **Agent 友好**：所有命令必须支持 `--format json`，输出结构化数据，方便 LLM 解析。

## 3. 工具生态定位
`codex-kit` 不是万能的，它与现有优秀 Rust 工具的边界如下：

| 需求场景 | 推荐使用的工具 | 说明 |
|---------|--------------|------|
| 内容搜索 (grep) | `rg` (ripgrep) | 极速，自动忽略 .gitignore |
| 文件名查找 (find) | `fd` | 极速，正则支持好 |
| 带语法高亮的查看 (cat) | `bat` | 自动分页，语法高亮 |
| **目录列表 (ls/dir)** | **`codex-kit ls`** | 统一跨平台输出格式 |
| **目录树 (tree)** | **`codex-kit tree`** | rg/fd 无此功能 |
| **文件元信息 (stat)** | **`codex-kit info`** | 统一跨平台权限/时间格式 |
| **系统环境探测** | **`codex-kit stat`** | 获取 OS、Python 版本等环境信息 |
| **安全文本替换** | **`codex-kit replace`** | 支持 dry-run，防止 LLM 误操作 |
| **文件头尾读取** | **`codex-kit head/tail`** | 大文件安全读取，不卡顿 |
| **文件统计与对比** | **`codex-kit wc/diff`** | 轻量级统计与差异对比 |
| **动态代码执行兜底** | **`codex-kit exec`** | 调用系统 Python 执行任意代码 |

## 4. 详细功能需求 (子命令设计)

### 全局参数
所有子命令均支持以下全局参数：
- `--format <plain|json>`：输出格式。默认 `plain`（人类可读，带颜色），`json` 输出严格的 JSON 对象到 stdout。

### 4.1 `ls [path]`
- **功能**：列出目录内容。
- **参数**：
  - `path`：目录路径，默认 `.`。
  - `-a, --all`：显示隐藏文件。
  - `-l, --long`：显示详细信息（权限、大小、修改时间）。
- **JSON 输出**：包含 `name`, `type` (file/dir/symlink), `size`, `modified` 等字段的数组。

### 4.2 `tree [path]`
- **功能**：以树状图展示目录结构。
- **参数**：
  - `path`：根目录，默认 `.`。
  - `-d, --depth <N>`：最大深度，默认 3。
  - `--ignore <pattern>`：忽略的 glob 模式（如 `node_modules`）。
- **JSON 输出**：嵌套的树形 JSON 对象。

### 4.3 `info <path>`
- **功能**：获取文件/目录的详细元信息。
- **JSON 输出**：包含 `path`, `exists`, `type`, `size_bytes`, `created`, `modified`, `readonly` 等字段。

### 4.4 `stat`
- **功能**：探测当前系统环境信息，帮助 LLM 了解运行上下文。
- **JSON 输出**：包含 `os`, `arch`, `hostname`, `cwd`, `path_dirs`, `python_version`, `node_version`, `git_version` 等字段。（如果某个工具未安装，对应字段返回 `null`）。

### 4.5 `replace <file> <old> <new>`
- **功能**：在文件中替换文本。必须先读取完整内容到内存，替换后再写回，防止截断。
- **参数**：
  - `--dry-run`：仅输出替换前后的 diff 预览，不修改文件。
  - `--regex`：将 `<old>` 视为正则表达式。
- **安全机制**：默认自动备份原文件为 `<file>.bak`，可通过 `--no-backup` 禁用。

### 4.6 `head <file>` / `tail <file>`
- **功能**：读取文件的前 N 行或后 N 行。
- **参数**：`-n <lines>`，默认 20。
- **注意**：必须使用流式读取，不能将整个大文件读入内存。支持自动编码检测。

### 4.7 `wc <file>`
- **功能**：统计文件的行数、字数、字节数。
- **JSON 输出**：`lines`, `words`, `bytes`, `chars`。

### 4.8 `diff <file1> <file2>`
- **功能**：对比两个文件的差异。
- **输出**：Plain 模式输出带颜色的 unified diff；JSON 模式输出结构化的变更块（hunks）。

### 4.9 `exec <code>`
- **功能**：兜底命令。调用系统级 Python 执行动态代码，用于处理预定义命令无法满足的复杂逻辑。
- **参数**：
  - `code`：Python 代码字符串。
  - `-f, --file`：将 `code` 参数视为 `.py` 文件路径。
- **行为**：透传 stdout/stderr，并返回 Python 进程的退出码。

## 5. 技术实现细节

### 5.1 依赖库选型 (Cargo.toml)
```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
walkdir = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
encoding_rs = "0.8"
chrono = { version = "0.4", features = ["serde"] }
similar = "2"
colored = "2"
glob = "0.3"
regex = "1"

5.2 多编码自适应读取策略 (核心难点)
Windows 下存在大量 GBK/GB2312 编码文件。读取文件内容时，必须实现以下降级策略：
尝试以 UTF-8 读取，若无 BOM 且无解码错误，则成功。
若 UTF-8 失败，使用 chardet 逻辑或直接尝试 GBK (通过 encoding_rs::GBK)。
若 GBK 失败，尝试 GB18030。
最终兜底使用 WINDOWS_1252 (Latin-1)，确保不会抛出解码异常。
5.3 错误处理规范
所有业务错误必须输出到 stderr，格式为：Error: <message>。
当 --format json 时，如果发生错误，stdout 应输出 {"error": "<message>"}，且进程退出码非 0。
严禁将错误信息输出到 stdout，以免破坏 JSON 解析。


6. 项目目录结构
text

编辑

codex-kit/
├── Cargo.toml
├── SPEC.md               # 本文件
├── README.md             # 用户文档
├── src/
│   ├── main.rs           # CLI 入口，Clap 路由
│   ├── cli.rs            # Clap 结构体定义
│   ├── commands/         # 各子命令实现
│   │   ├── mod.rs
│   │   ├── ls.rs
│   │   ├── tree.rs
│   │   ├── info.rs
│   │   ├── stat.rs
│   │   ├── replace.rs
│   │   ├── head_tail.rs
│   │   ├── wc.rs
│   │   ├── diff.rs
│   │   └── exec.rs
│   └── utils/            # 公共工具
│       ├── mod.rs
│       ├── encoding.rs   # 多编码读取
│       └── output.rs     # Plain/JSON 输出格式化器


7. 构建与发布

# 开发阶段
cargo run -- ls .

# 生产构建 (Windows)
cargo build --release
# 产物位于 target/release/codex-kit.exe

# 生产构建 (跨平台优化)
# 在 Cargo.toml 中配置：
# [profile.release]
# opt-level = 3
# lto = true
# strip = true
# codegen-units = 1

AGENTS.md

### 系统工具与环境说明
当前环境已安装 `codex-kit` 跨平台工具集，以及 `rg` (ripgrep) 和 `fd`。
在执行系统操作时，请严格遵守以下路由规则：

1. **内容搜索**：使用 `rg "pattern" [dir]`
2. **文件查找**：使用 `fd "pattern" [dir]`
3. **目录与文件操作**：使用 `codex-kit` 命令：
   - 列表：`codex-kit ls [path]`
   - 树状图：`codex-kit tree [path]`
   - 元信息：`codex-kit info <path>`
   - 环境探测：`codex-kit stat`
   - 文本替换：`codex-kit replace <file> <old> <new> [--dry-run]`
   - 文件头尾：`codex-kit head/tail <file>`
4. **动态代码执行**：当上述工具无法满足时，使用 `codex-kit exec "python_code"` 调用系统 Python。
5. **禁止事项**：不要使用 PowerShell 原生的 `dir`, `ls`, `Select-String`，不要使用 `cat` 读取大文件，不要试图使用项目虚拟环境执行系统操作。
6. **输出解析**：对于复杂结果，请在命令后追加 `--format json` 以获取结构化数据。


---

### 给你的额外建议：

1. **先让 Agent 搭骨架**：Agent 读完这个文档后，让它先只生成 `Cargo.toml`、`src/main.rs` (包含所有 clap 定义) 和 `src/utils/`。你运行 `cargo check` 确认没报错，再让它写具体的 `commands` 实现。
2. **重点关注 `encoding.rs`**：这是 Windows 下最容易踩坑的地方，让 Agent 写完后，你一定要找个 GBK 编码的中文文件实际测一下 `codex-kit head <gbk_file>`。
3. **关于 `exec` 命令**：这是整个工具的"灵魂兜底"。如果 Agent 遇到它不知道怎么处理的复杂系统操作，它可以自己用 `codex-kit exec "..."` 写一段 Python 来解决，这就让你的工具集具备了**图灵完备性**。