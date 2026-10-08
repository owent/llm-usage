# Terminal environment and tools

<a id="终端环境与工具"></a>

<a id="选择与本地探测"></a>

## Selection and local probes

Prefer installed modern CLIs suited to the task: rg for text search, rg --files for repository enumeration,
fd for filtering, bat for reading, sd for suitable replacements, jq for JSON and verified Mike Farah yq for YAML.
Follow the harness's native reading/patch interfaces. Fall back correctly when tools are missing, platforms
differ or options/semantics are incompatible. Enumerate real paths before reading; verify temporary script
argument requirements rather than guessing names/repeatedly trying paths. Do not switch to traditional tools
out of habit, install in bulk or rewrite stable scripts for tool preference. Performance claims need real
data, not implementation languages or marketing benchmarks.

Verified in this Windows session on 2026-09-24 with Get-Command -CommandType Application and version commands:

| Tool | Path or availability | Actual version/fallback |
| --- | --- | --- |
| PowerShell | WindowsApps pwsh.exe, launched by the harness | 7.6.6; Windows argument mode |
| rg | C:/Users/owt50/scoop/shims/rg.exe | 15.2.0; Select-String if absent |
| jq | C:/Users/owt50/scoop/shims/jq.exe | 1.8.2; ConvertFrom-Json if absent |
| yq | C:/Users/owt50/scoop/shims/yq.exe | Mike Farah v4.53.6; project YAML library if absent |
| fd | Not found on PATH | rg --files or Get-ChildItem -LiteralPath |
| sd | Not found on PATH | Harness patches; verify encoding/replacement scope first |
| bat | Not found on PATH | Harness reading or Get-Content -LiteralPath |
| Node.js | scoop/apps/nodejs-lts/current/node.exe | 24.21.0; documentation lint needs 22+ |
| Python | scoop/apps/python/current/python.exe | 3.14.7; temporary checks, not project runtime |

Absolute paths are machine snapshots, not shared script constants. Reprobe other environments as needed.
Initial sandbox startup failed with CreateProcessAsUserW failed: 5; a platform-permitted retry restored
execution. Actual shell versions come from output. Infrastructure errors are not product test results.

<a id="catalog"></a>

<a id="完整候选清单"></a>

## Complete candidate catalog

All 31 user-provided candidates and limits are retained. This is a selection list, not an installation list.
Verified adoption of rg, jq, yq and PowerShell is recorded in [sources](source-index.md). Other candidates
were neither installed nor run in the original task; verify platforms, arguments, versions and supply chains
before actual use. Links identify official/maintainer entry points, not verification of every candidate.
plocate links only to the supplied historical archive.

| Modern tool and source | Purpose and preferred use | Traditional equivalent, fallback and limits |
| --- | --- | --- |
| [ripgrep (`rg`)](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md) | Preferred text search; `rg --files` enumerates repository files | Compared with `grep`; fall back to `Select-String`; account for ignores, hidden files and binaries |
| [ugrep](https://github.com/Genivia/ugrep) | Its specific search features or grep-compatible options | Compared with `grep`; verify options individually, never promise complete equivalence; avoid TUI in automation |
| [fd](https://github.com/sharkdp/fd) | Filter names, paths and types | Compared with `find`; fall back to `Get-ChildItem`; check hidden files, ignores and regex/literal differences |
| [bat](https://github.com/sharkdp/bat) | Read files, line numbers and ranges | Compared with `cat`; native reading/`Get-Content` fallback; disable paging/color in automation |
| [sd](https://github.com/chmln/sd) | Replacements suited to its string/regex semantics | Compared with `sed`; verify scope, captures and encoding; patches still use harness interfaces |
| [eza](https://github.com/eza-community/eza) | Directory listings/tree browsing | Compared with `ls`/`tree`; verify platform, bound depth and never parse icons/colors as data |
| [erdtree (`erd`)](https://github.com/solidiquis/erdtree) | Directory trees with disk usage | Compared with `tree`/`du`; bound scans and distinguish presentation from precise storage statistics |
| [dust](https://github.com/bootandy/dust) | Locate large files/directories and usage distribution | Compared with `du`; distinguish logical/allocated size and permission-dependent statistics |
| [duf](https://github.com/muesli/duf) | Filesystem capacity/mounts | Compared with `df`; container/network/mount visibility depends on actual environment |
| [hyperfine](https://github.com/sharkdp/hyperfine) | Repeat timing measurements/compare options | Compared with `time`/`Measure-Command`; control warmup/cache; Windows defaults to `cmd.exe`, choose `--shell pwsh` for PowerShell semantics |
| [tokei](https://github.com/XAMPPRocky/tokei) | Count code/comments/blank lines by language | Compared with `cloc`; record exclusions/counting rules; lines are not quality |
| [hexyl](https://github.com/sharkdp/hexyl) | Bounded hexadecimal binary inspection | Compared with `xxd`/`hexdump`; verify conversion/edit tools separately; avoid sensitive output |
| [jq](https://jqlang.org/manual/) | Default JSON queries/transforms/conditions | Project parsers/PowerShell JSON fallback; preserve types/exit semantics, never parse JSON with regex |
| [jaq](https://github.com/01mf02/jaq) | jq alternative for verified compatible filters | No complete jq equivalence; verify actual filters/modules/errors |
| [yq (Mike Farah)](https://github.com/mikefarah/yq) | Query/edit YAML and other structured formats | Verify implementation/version; other yq options differ; check comments/style/data after writing |
| [Miller (`mlr`)](https://github.com/johnkerl/miller) | Filter/transform CSV, TSV and JSON records | Compared with `awk`/`cut` pipelines; respect formats/quoting/types, never split CSV directly on commas |
| [qsv](https://github.com/dathere/qsv) | CSV processing/validation/statistics | Compared with CSV scripts/text pipelines; verify distribution variants, subcommands/resources; do not assume constant memory |
| [git-delta (`delta`)](https://github.com/dandavison/delta) | Human-readable diffs | Compared with ordinary diff display; disable paging for automation, retain raw `git diff` for machines |
| [difftastic (`difft`)](https://github.com/Wilfred/difftastic) | Syntax-aware comparison for supported languages | Compared with line diff; structure assists review, scope/patches still check `git diff` |
| [lnav](https://docs.lnav.org/en/latest/cli.html) | Aggregate/search/analyze logs | Compared with `less`/`tail`; use noninteractive `-n` and bounded queries |
| [tailspin (`tspin`)](https://github.com/bensadeh/tailspin) | Highlight logs for inspection | Compared with `tail`/`less`; verify paging/follow/color; machines prefer raw logs |
| [plocate](https://sources.debian.org/src/plocate/1.1.18-1/README) | Use existing Linux filename indexes | Compared with `locate`; depends on coverage/freshness, check files after matches; otherwise `fd`/`rg --files`, no whole-disk indexing for local searches |
| [pigz](https://zlib.net/pigz/) | Parallel compression when gzip is required | Compared with `gzip`; compression gains do not establish parallel decompression gains; check threads/memory |
| [zstd](https://github.com/facebook/zstd) | Compression when consumers support Zstandard | Compared with `gzip`/`xz`; choose using format requirements/compression ratio/time/memory, never change artifact formats implicitly |
| [ouch](https://github.com/ouch-org/ouch) | Unified supported compression/archive interface | Compared with `tar`/`unzip`; verify formats/options and destination/path safety before extraction |
| [aria2 (`aria2c`)](https://aria2.github.io/) | Downloads/resume/suitable concurrency | Compared with download uses of `wget`/`curl`; respect limits/check artifacts, no credentials in args/logs |
| [fzf](https://github.com/junegunn/fzf) | Fuzzy candidate filtering | Automation uses `--filter`; exact matching prefers `rg`; no TTY selection waits |
| [xh](https://github.com/ducaale/xh) | Suitable HTTP requests/debugging | Compared with `curl`/HTTPie; verify arguments/body/auth/redirects/failure exit codes |
| [doggo](https://github.com/mr-karan/doggo) | DNS queries/troubleshooting | Compared with `dig`/`nslookup`; specify resolver/type/transport |
| [procs](https://github.com/dalance/procs) | Filter/view processes | Compared with `ps`; `Get-Process` fallback; platform/fields depend on version |
| [watchexec](https://github.com/watchexec/watchexec) | Watch files/run development commands | Compared with polling; use for intentionally persistent tasks, ignore appropriate paths and manage children/cleanup |

<a id="通用与-windows-执行合同"></a>

<a id="通用与-windows-执行要求"></a>

<a id="general-and-windows-execution-contract"></a>

## General and Windows execution requirements

General rules:

- Common candidates are `rg`, `fd`, `sd`, `jq`, Mike Farah `yq` and `bat`. Check platform availability
  during initialization; ordinary work probes only needed tools and refreshes after environment changes.
  Explicit executables prevent alias/implementation ambiguity.
- Prefer suitable installed modern tools while respecting harness native reading/patches and stable script
  compatibility. Do not rewrite unrelated scripts for preferences.
- Obtain tools from official artifacts or documented trusted package channels. Verify platform, architecture,
  version and available signatures/checksums. Install necessary missing tools within authorization as needed;
  fall back for optional tools rather than installing the whole catalog.
- `cargo binstall` may fall back to `cargo install` when binaries are unavailable. When local compilation
  is forbidden, explicitly select/verify a binary-only policy. mise/aqua do not universally guarantee no
  compilation. [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)
- Prefer structured, bounded output with color/paging disabled. Noninteractive `fzf` uses `--filter`;
  avoid TTY workflows.
- `rg --max-count` limits matching lines per file, not total output. Ignores/hidden/binary files affect
  search completeness. [ripgrep](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)
- Interpret exit codes per tool: `rg` 1 normally means no match; `jq -e` returns 1 for final false/null
  and 4 for no valid output, not always 1. [jq](https://jqlang.org/manual/)
- Pass arguments as arguments. JSON serialization is not shell quoting. Do not interpolate web/user text
  into executable code or print complete commands containing secrets.

Windows rules:

- Prefer PowerShell 7+ (`pwsh`) after checking actual version/harness support. Use verified alternatives
  when restrictions offer other shells and report differences; do not mix shells within an operation.
- Separate processes use `-NoLogo -NoProfile`, adding `-NonInteractive` as needed. Explicit executables/
  full cmdlet names avoid aliases such as `where`/`curl`.
- Use single quotes without interpolation and double quotes only when needed; multiline text uses
  here-strings. Paths prefer `-LiteralPath`; pipe statement-block output with `& { ... } | ...`.
- Native commands can use argument-array splatting. PowerShell 7.3+ still depends on
  `$PSNativeCommandArgumentPassing`; Windows mode falls back to Legacy for certain programs/scripts,
  so quoting is not universally automatic. `--%` has platform/expansion limits and is not a general fix.
  [Parsing](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-7.5)
- Shared new text specifies UTF-8. Preserve existing BOM/newlines/final newlines where practical.
  Windows PowerShell 5.1 defaults vary by command; do not claim every write is UTF-16LE.
  [Encoding](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding?view=powershell-7.5)
- Critical cmdlets use `-ErrorAction Stop`. Save `$LASTEXITCODE` immediately after native commands,
  interpret the command’s documented exit codes, then run the next command.
- Helpers should not unexpectedly show windows. Use `Start-Process -WindowStyle Hidden` as appropriate
  and track PIDs/logs/cleanup.
- Separate launcher/sandbox infrastructure from script failures. Use platform permission retries rather
  than weakening security.

<a id="本次回退与校验"></a>

## Fallbacks and verification in this task

The original task used rg enumeration/search, jq/yq structured checks and harness patches. Missing fd/sd/bat
fell back to rg/PowerShell, patches and Get-Content respectively. Disposable deterministic scripts handled
complex coverage/link checks without replacing search tools. jq -e false/null returns 1, no output 4;
rg no matches returns 1; report errors separately. Necessary documentation tooling used locked npm packages/
integrity checks with installation scripts disabled. Unrelated CLIs were not installed. Verify then-current
cargo-binstall binary policies before use; mise/aqua do not always avoid local compilation.
