# synthetic-unknown-version._expectations.md（SYNTHETIC）

<a id="synthetic-unknown-version_expectationsmd-synthetic"></a>

protocol_version="1.5"：kimi-code 注册表已验证、kimi-work 注册表只锚定 1.4——
同一文件在 kimi-work 侧必须走 latest_fallback（A12/A13 注册表独立）。
期望：latest_fallback 诊断 1 条；1 事件照常入账（parse_basis=latest_fallback）；
usage {100,50,400,0}：input_total=500、total_tokens=550；回声不双计。
