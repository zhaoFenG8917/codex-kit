# System Tool Routing (codex-kit)

This environment has `codex-kit`, `rg`, `fd` and `bat` installed.
Route system operations as follows:

1. Content search: `rg "pattern" [dir]`. File finding: `fd "pattern" [dir]`.
2. Directory & file operations via `codex-kit`:
    `ls` / `tree` / `info` / `stat` / `replace` / `head` / `tail` / `wc` / `diff` / `read` /
    `ps` / `kill` / `port` / `run` / `which` / `write` / `hash` / `archive` / `extract` / `http`.
3. Text files on Windows may be GBK-encoded — prefer `codex-kit head/tail/wc/replace`
   (automatic encoding detection) over raw shell reads.
4. For risky replacements run `codex-kit replace <file> <old> <new> --dry-run` first.
5. Escape hatch: `codex-kit exec "python_code"` runs the **system** Python.
   Never use a project's virtualenv interpreter for system operations.
6. Append `--format json` whenever you need to parse the output.
7. The command list above may lag the installed version. Run `codex-kit --help`
   (or `codex-kit <command> --help`) to discover the actual commands and flags —
   trust the `--help` output over this list.
