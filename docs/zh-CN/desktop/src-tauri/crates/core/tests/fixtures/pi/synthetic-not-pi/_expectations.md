# synthetic-not-pi._expectations.md（SYNTHETIC）

<a id="synthetic-not-pi_expectationsmd-synthetic"></a>

**本目录全部为合成样本（synthetic），不是真实会话提取。** 覆盖 V17：非 pi 格式
fail closed（首行不是 session 头）。

<a id="scenario-and-expectations"></a>

## 场景与期望

- 首行 `{"type":"event_msg",...}`（仿 codex rollout 形状；第二行即使放了合法 session
  头也不被采纳，detect 只看首行）。
- detect → UnknownFormat("first record type is not session header")。
- 整轮运行：status=unknown_format、0 事件；diagnostics 表 unknown_format ×1；
  source_files status=unsupported。
