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
| **进程管理** | **`codex-kit ps/kill`** | 进程列表查询与按 PID 结束进程 |
| **端口探测** | **`codex-kit port`** | 检查 TCP 端口监听状态及占用进程 |
| **行区间读取** | **`codex-kit read`** | 按行号范围流式读取，适合大文件局部查看 |
| **命令执行** | **`codex-kit run`** | 带超时控制与退出码透传的任意命令执行 |
| **后台服务** | **`codex-kit start`** | 脱离会话拉起长驻进程，日志落盘，可等端口就绪 |
| **命令定位** | **`codex-kit which`** | PATH 查找与版本探测（Windows 自动处理 PATHEXT） |
| **归档压缩** | **`codex-kit archive/extract`** | zip / tar.gz 创建与解压（防路径穿越） |
| **编码写入** | **`codex-kit write`** | 将 stdin 以指定编码（utf8/gbk/gb18030）写入文件 |
| **文件校验** | **`codex-kit hash`** | 流式计算 md5 / sha256 校验和 |
| **HTTP 请求** | **`codex-kit http`** | 跨平台替代 curl/Invoke-RestMethod，状态码视为数据 |
| **安全删除** | **`codex-kit rm/trash/restore`** | 默认移入回收站可还原，替代 Remove-Item/rm -rf |
| **文件移动** | **`codex-kit mv`** | 跨卷移动/重命名，默认不覆盖目标 |
| **文件下载** | **`codex-kit download`** | 流式下载 URL 到文件，替代 Invoke-WebRequest/curl |
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
  - `--limit <N>`：只输出前 N 个条目。
- **JSON 输出**：包含 `name`, `type` (file/dir/symlink), `size`, `modified` 等字段的数组；指定 `--limit` 时输出 `{entries, total, returned, truncated}`，截断后 JSON 依然合法。

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
- **输出**：Plain 模式输出带颜色的 unified diff；JSON 模式输出结构化的变更列表（含操作类型与行号）。

### 4.9 `exec <code>`
- **功能**：兜底命令。调用系统级 Python 执行动态代码，用于处理预定义命令无法满足的复杂逻辑。
- **参数**：
  - `code`：Python 代码字符串。
  - `-f, --file`：将 `code` 参数视为 `.py` 文件路径。
- **行为**：透传 stdout/stderr，并返回 Python 进程的退出码。

### 4.10 `ps [name]` / `kill <pid>`
- **功能**：`ps` 列出系统进程（可按名称不区分大小写过滤，按内存降序）；`kill` 按 PID 结束进程。
- **参数**：`ps` 支持 `--limit <N>` 只取内存占用前 N 个进程；`kill` 支持 `--tree` 连杀整棵进程树（先杀目标再清扫预收集的后代，解决 cmd/npm 包装进程杀父留子的问题）。
- **安全护栏**：拒绝杀死 pid 0/4、自身及自身祖先链进程；进程树后代超过 64 个时拒绝执行；杀失败后会复查进程是否已自行退出（包装进程常在子进程死亡时自动退出，视为成功）。
- **健壮性**：进程枚举使用轻量刷新（不读取每个进程的完整信息，秒级返回）；祖先链与后代遍历带环路检测（Windows PID 复用可能形成父指针环）；整体操作有 20 秒看门狗，超时返回明确错误而不是无限卡死。
- **JSON 输出**：`ps` 输出包含 `pid`, `name`, `exe`, `memory_bytes`, `started` 的数组；`kill` 输出 `{pid, name, killed, tree_killed[], tree_failed[]}`。

### 4.11 `port <port>`
- **功能**：检查 TCP 端口。本机（默认 127.0.0.1）报告监听状态与占用进程（Windows 解析 `netstat -ano`，Unix 使用 `lsof`）；指定远程主机时探测连通性并测量握手延迟，替代 `Test-NetConnection`。
- **参数**：
  - `--host <主机>`：目标主机（IP 或域名，自动 DNS 解析），默认 127.0.0.1。
  - `--timeout <ms>`：连接超时毫秒数，默认 1000。
  - `--kill`：直接结束本机监听进程（停 dev server 一条命令：`codex-kit port 5173 --kill`），与 `kill` 共用同一套安全护栏。
  - `--tree`：配合 `--kill` 连杀监听者的整棵进程树。
- **JSON 输出**：`{host, port, listening, latency_ms, pid, process_name, kill?}`（pid/process_name 仅本机有效）。

### 4.12 `read <file> --range <range>`
- **功能**：按 1 起始的行号区间流式读取文件，跳过区间前、读完区间后即停止，不加载整个文件。
- **range 格式**：`100:200`、`100:`（到文件尾）、`:200`（从开头）、`150`（单行）。
- **JSON 输出**：`{file, encoding, start, end, lines[]}`。

### 4.13 `run [--timeout N] <cmd...>`
- **功能**：带超时（默认 30 秒）执行任意外部命令，stdout/stderr 在独立线程捕获（无管道死锁），输出自动解码。
- **参数**：`--unset <KEY>` 为子进程删除环境变量（可重复，如清掉 HTTP_PROXY 等代理变量）；`--env <KEY=VAL>` 为子进程设置环境变量（可重复）。
- **行为**：透传子进程退出码；超时返回 124。注意 `--timeout` 等选项须写在被执行命令之前。

### 4.14 `which <command>`
- **功能**：在 PATH 中定位命令（Windows 自动尝试 PATHEXT 扩展名），并探测 `--version`（2 秒超时）。
- **JSON 输出**：`{command, found, path, version}`；未找到时报错并以非零码退出。

### 4.15 `archive <output> <inputs...>` / `extract <archive> [-d dest]`
- **功能**：按扩展名识别格式（`.zip` 或 `.tar.gz`/`.tgz`），创建或解压归档；输入可以是文件或目录（目录递归打包，以目录名为归档内根）。
- **安全性**：解压时拒绝路径穿越条目（zip-slip）。

### 4.16 `write <file>`
- **功能**：将 stdin 内容以指定编码写入文件，解决 Agent 在 Windows 下无法可靠创建 GBK 文件的问题。
- **参数**：`--encoding <utf8|gbk|gb18030>`（默认 utf8），`--append` 追加模式；自动创建缺失的父目录。

### 4.17 `hash <file>`
- **功能**：以 64KB 块流式计算文件校验和，大文件不占内存。
- **参数**：`-a, --algorithm <sha256|md5>`，默认 sha256。

### 4.18 `http <url>`
- **功能**：跨平台 HTTP 客户端，替代 curl / Invoke-RestMethod / Invoke-WebRequest，避免 PowerShell 语法差异与别名陷阱。
- **参数**：
  - `-X, --method <M>`：HTTP 方法，默认 GET。
  - `-H, --header <"Name: value">`：请求头，可重复。
  - `-d, --data <body>`：请求体。
  - `--timeout <N>`：超时秒数，默认 30。
- **行为**：收到任何 HTTP 响应（含 4xx/5xx）都以退出码 0 返回，状态码作为数据输出；仅传输层失败（DNS、连接拒绝、超时）返回非零。响应体按服务器 charset 头解码（GBK 安全）。
- **JSON 输出**：`{url, method, status, duration_ms, headers{}, body}`。

### 4.19 `start <cmd...>`
- **功能**：以后台守护方式拉起长驻服务进程，替代 `Start-Process -WindowStyle Hidden` / `nohup ... &`。进程脱离调用方会话独立存活（Windows 无控制台窗口、独立进程组；Unix 独立进程组防 SIGHUP）。
- **参数**：
  - `--name <label>`：服务标签，用于默认日志文件名。
  - `--cwd <dir>`：服务的工作目录。
  - `--log <file>`：日志文件路径（默认：临时目录下自动命名），stdout/stderr 均追加写入。
  - `--wait-port <port>`：启动后等待该 TCP 端口开始监听再返回。
  - `--wait-timeout <N>`：`--wait-port` 的等待秒数，默认 60。
  - `--unset <KEY>` / `--env <KEY=VAL>`：为服务进程删除/设置环境变量（可重复）。
- **行为**：
  - 启动即返回 `{pid, name, cmd, log, detached, wait_port, ready}`；不持有子进程。
  - `--wait-port` 等待期间若进程提前退出，报错并提示日志路径；超时未监听则 `ready=false`（进程仍在运行），退出码仍为 0，由调用方检查 `ready` 字段。
  - Windows 下 spawn 前会临时摘除自身 std 句柄的继承标志，防止守护进程持有调用方管道导致调用方永远读不到 EOF（句柄泄漏挂起）。
- **配套**：停止用 `codex-kit kill <pid>`，状态用 `codex-kit port <port>`，排障用 `codex-kit tail <log>`。

### 4.20 `rm <paths...>` / `trash` / `restore <ids...>`
- **功能**：安全删除体系，替代 `Remove-Item` / `rm -rf`（永久删除且语法跨平台不一致）。
- **设计原则**：默认一切可逆，不可逆操作必须显式声明。
- **`rm`**：
  - 默认把文件/目录移动到回收站（`~/.codex-kit/trash/<id>/`，含 `meta.json` 记录原路径、删除时间、大小；跨卷自动降级为复制+删除）。
  - `--force`：永久删除。
  - `--dry-run`：只预览将发生什么。
  - 拒绝删除回收站目录本身。
- **`trash`**：列出回收站（ID、大小、删除时间、原路径，新的在前）；`--empty` 永久清空。
- **`restore`**：按 ID 还原到原路径（`--all` 全部还原）；原路径已存在时报错拒绝（保护新文件），`--overwrite` 显式覆盖。
- **JSON 输出**：`rm`/`restore` 输出逐项结果数组（含 error 字段，部分失败时退出码非零）；`trash` 输出元数据数组。

### 4.21 `mv <src> <dst>`
- **功能**：移动/重命名文件或目录，跨卷自动降级为复制+删除；目标是已存在目录时移入该目录（Unix mv 语义）。
- **安全性**：目标已存在时报错拒绝，`--force` 显式覆盖。

### 4.22 `download <url>`
- **功能**：把 URL 下载到文件（二进制安全，不做文本解码），替代 `Invoke-WebRequest -OutFile` / `curl -O`，并做多线程与断点续传增强。
- **参数**：
  - `-o, --output <file>`：输出路径（默认取 URL 末段文件名，存当前目录）。
  - `--threads <N>`：并行分段数，默认 4；服务器支持 Range 且文件 ≥8MB 时生效，否则自动降级单流。
  - `--resume`：断点续传。单流模式按已有字节发 `Range: bytes=N-` 追加；并行模式以 `<output>.ckparts/part-N` 的文件长度作为续传位置，重跑同参数命令即可继续。
  - `--timeout <N>`：单请求超时秒数，默认 120。
  - `--force`：删除已有输出与分段缓存，从头下载。
- **行为**：
  - 先 HEAD 探测（不行则 `Range: bytes=0-0` 探测）获取总大小与 Range 支持。
  - 分段下载写入 `<output>.ckparts/`（含 meta.json 记录 url/大小/线程数，计划不一致自动作废重下），全部完成后拼接并清理。
  - 分段计划变更（URL/大小/线程数不同）时旧 part 自动作废，不会拼错数据。
  - 非 2xx 响应（如 404）直接报错且不产生残留文件；输出文件已存在且未给 `--resume`/`--force` 时报错。

## 5. 技术实现细节

### 5.1 依赖库选型 (Cargo.toml)

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
walkdir = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
encoding_rs = "0.8"
chardetng = "0.1"
chrono = { version = "0.4", features = ["serde"] }
similar = "2"
colored = "2"
glob = "0.3"
regex = "1"
```

### 5.2 多编码自适应读取策略 (核心难点)

Windows 下存在大量 GBK/GB2312 编码文件。读取文件内容时，必须实现以下降级策略：

1. 尝试以 UTF-8 读取，若无 BOM 且无解码错误，则成功。
2. 若 UTF-8 失败，使用 chardetng 检测编码，或直接尝试 GBK（通过 `encoding_rs::GBK`）。
3. 若 GBK 失败，尝试 GB18030。
4. 最终兜底使用 WINDOWS_1252 (Latin-1)，确保不会抛出解码异常。
5. 写回文件时（如 `replace`）必须按原编码重新编码，并保留原始 BOM，避免改变文件编码。

### 5.3 错误处理规范

- 所有业务错误必须输出到 stderr，格式为：`Error: <message>`。
- 当 `--format json` 时，如果发生错误，stdout 应输出 `{"error": "<message>"}`，且进程退出码非 0。
- 严禁将错误信息输出到 stdout（plain 模式），以免破坏 JSON 解析。

## 6. 项目目录结构

```text
codex-kit/
├── Cargo.toml
├── Cargo.lock
├── SPEC.md               # 本文件
├── README.md             # 用户文档（英文）
├── README.zh-CN.md       # 用户文档（中文）
├── LICENSE
├── AGENTS_TEMPLATE.md    # Agent 集成模板
├── .github/
│   └── workflows/
│       └── release.yml   # 三平台 CI/CD
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
```

## 7. 构建与发布

```bash
# 开发阶段
cargo run -- ls .

# 生产构建 (Windows)
cargo build --release
# 产物位于 target/release/codex-kit.exe
```

跨平台优化在 Cargo.toml 中配置：

```toml
[profile.release]
opt-level = 3
lto = true
strip = true
codegen-units = 1
```

发布由 GitHub Actions 自动完成（见 `.github/workflows/release.yml`）：推送 `v*` 格式的 tag 即触发 Windows / Linux / macOS 三平台构建，并自动创建 Release 上传产物。

## 8. AGENTS.md 集成

将以下路由规则加入 `~/.codex/AGENTS.md`（全局生效）或项目根目录的 `AGENTS.md`（项目级），Agent 即会自动把系统操作路由到 codex-kit。仓库中的 `AGENTS_TEMPLATE.md` 为可直接使用的英文模板。

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

## 附录：实施过程回顾

本项目由 AI Agent 按以下流程实现，已被验证有效：

1. **先搭骨架**：先生成 `Cargo.toml`、`src/main.rs`（包含所有 clap 定义）和 `src/utils/`，`cargo check` 通过后再实现各 `commands`。
2. **重点验收 `encoding.rs`**：这是 Windows 下最容易踩坑的地方——用 GBK 编码的中文文件实测 `codex-kit head <gbk_file>` 与 `replace` 写回。本项目验收已通过：GBK 文件正确识别、替换后文件编码保持不变。
3. **`exec` 是灵魂兜底**：当 Agent 遇到预定义命令无法处理的复杂系统操作时，可用 `codex-kit exec "..."` 调用系统 Python 解决，这让工具集具备了图灵完备性。
