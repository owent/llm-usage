# synthetic-bad-shapes._expectations.md（SYNTHETIC）

<a id="synthetic-bad-shapes_expectationsmd-synthetic"></a>

1 条正常 turn {200,60,800,10} 入账（input_total=1010、total=1070）；
负值 inputOther=-5 ⇒ usage_shape_deviation 跳过；
usageScope="lifetime"（未知 scope）⇒ usage_scope_unknown 跳过；
time=1767225600（秒级毫秒位，早于 MIN_PLAUSIBLE_MS）⇒ timestamp_unparseable
跳过且**不做 ×1000 猜测**（实读 82 文件全部毫秒，无秒样本）。
期望：1 事件；3 诊断；文件 degraded。
