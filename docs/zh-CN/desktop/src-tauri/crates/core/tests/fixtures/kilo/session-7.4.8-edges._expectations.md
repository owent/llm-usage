# session-7.4.8-edges._expectations.md 期望值（独立核算）

<a id="session-748-edges_expectationsmd-independently-calculated-expectations"></a>

核算方法：直接对脱敏 JSON 的 assistant 消息逐条求和；
total 计算规则 = input+output+reasoning+cache.read+cache.write（全互斥）。

- session anon-1 ver=7.4.8 main(primary): msgs=13 input=33638 output=1535 reasoning=4403 cache_read=272896 cache_write=0 derived_total=312472
  reported_total_sum=312472 | snapshot(in+out+reason+cr+cw)=312472.0 reconcile=matched models={'k2p7': 2, 'glm-5.2': 11}

- 合计: assistant msgs=13 input=33638 output=1535 reasoning=4403 cache_read=272896 cache_write=0 derived_total=312472 reported_total_sum=312472
  missing_total_msgs=2
