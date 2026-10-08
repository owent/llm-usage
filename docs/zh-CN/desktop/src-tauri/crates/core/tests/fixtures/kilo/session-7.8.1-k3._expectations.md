# session-7.8.1-k3._expectations.md 期望值（独立核算）

<a id="session-781-k3_expectationsmd-expected-values-independent-calculation"></a>

来源：本机实读 kilo.db `session.version=7.8.1` 单会话，脱敏（ids/paths/正文去除，
tokens/time/finish/modelID/providerID 保留）。提取脚本
`build/dashboard-fixes/extract_kilo_781.py`。

核算方法：对脱敏 JSON 的 assistant 消息逐条求和；
total 计算规则 = input+output+reasoning+cache.read+cache.write（全互斥）。
顶层 modelID/providerID 字段格式（仅登记已核验的 7.8.1，与 7.4.8/7.4.9 共用实现；
不据此核验其他 7.8.x）。2026-10-03 从只读连接进行 SQLite Online Backup，
重新核对该版本真实 assistant 字段形状与以下合计，避免 immutable 读取遗漏 WAL。

- session anon-1 ver=7.8.1 main(primary): assistant_msgs=34（+1 user，records_seen=35）
  input=125298 output=6637 reasoning=37853 cache_read=3019520 cache_write=0 derived_total=3189308
  reported_total_sum=3189308 missing_total_msgs=0 finish=tool-calls（无 error）
  snapshot(in+out+reason+cr+cw)=3189308 reconcile=matched models={'k3-256k': 34}

派生指标（map_kilo 计算规则）：

- input_total = input + cache_read + cache_write = 125298 + 3019520 + 0 = 3144818
- output_total = output + reasoning = 6637 + 37853 = 44490
- total_tokens = input_total + output_total = 3144818 + 44490 = 3189308
- cache_read_known = 3019520
