# synthetic-unsupported-version._expectations.md（SYNTHETIC）

<a id="synthetic-unsupported-version_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 目录名
"unsupported-version" 是历史样本组织（V17 时代 fail closed 场景），保留不改名；
V30 起 omp 未知版本策略改为 latest_fallback 兼容回退，本文件与断言反映新行为。

<a id="scenario-and-expectations-v30-unregistered-or-absent-version-attempts-the-latest-parser"></a>

## 场景与期望（V30：未收录/缺失 version 都回退最新内置解析器尝试）

omp 旧版保存格式未核验（尚未确认格式差异），与 pi 不同：pi 的 v1/v2 已由固定源码
确认不兼容，仍 fail closed；omp 尚未确认不兼容，未收录数值版本与 `version` 字段缺失都按
`LatestFallback` 回退 `session_v3` 尝试，不直接拒绝；结构不兼容由扫描层 V30
判定（读到记录、零事件且带结构诊断 ⇒ 保留旧结果）。

- `...d020.jsonl`（3 行）：session 头 `version=4`（本机仅核验过 3）⇒ detect
  `Supported{format=omp-session-jsonl, format_version=Some("4"),
  basis=latest_fallback}`。
- `...d021.jsonl`（3 行）：session 头无 `version` 字段（legacy 形状；条目 id 为
  `syn-l1`，与 d020 的 `syn-m1` 区分——V17 时代两文件同为 `syn-m1`，因零事件
  不冲突；回退语义下会构成同键不同内容的假冲突，改为独立条目）⇒ detect
  `Supported{format_version=None, basis=latest_fallback}`。
- 整轮：两文件均扫描成功（file status="complete"、各 1 事件
  100/10/0/0/110）、detail 带
  `latest_fallback: version compatibility unverified (found: 4|missing)`；
  usage_events 2 行，`parse_basis=latest_fallback`，schema_version 分别为
  "4" 与 "unknown"（头无版本无从得知）；source_files 两行均
  status="active_compat"，format_status basis=latest_fallback、
  compat=unverified、found_version 分别 "4" 与 null；diagnostics 表
  `latest_fallback` ×2（框架层逐文件一条）。
- 汇总（2026-01-05，UTC）：call_count=2、input_total=200（派生
  input+cacheRead+cacheWrite，各 100）、output=20、total_tokens=220。
- 再次扫描没有新增记录（游标已消费，added=0）。
