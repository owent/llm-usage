# synthetic-error-aborted._expectations.md（SYNTHETIC）

**本目录全部为合成样本（synthetic），不是真实会话提取。** stopReason=error/aborted
映射 error_status。本机真实核验结果：147 条 error/aborted 全带 usage；aborted 条目
无 duration/ttft 字段（k3 真实 fixture 同形状）。

## 场景与期望

- 4 行：title、session（version=3，id=syn-omp-err）、assistant syn-e-1
  （stopReason=error，usage 15/20/5/0/40，duration=100.4→100，ttft 缺字段 None）、
  assistant syn-e-2（stopReason=aborted，usage 5/5/0/0/10，duration/ttft 均缺）。
- 整轮：1 文件 complete、2 事件全 primary、added=2；
  error_status 分别为 error / aborted。
- 汇总（2026-01-05 UTC）：call_count=2、input_total_known=25（派生 (15+5+0)+(5+0+0)）、
  cache_read_known=5、cache_write_known=0、output_total_known=25、
  total_tokens_known=50。
