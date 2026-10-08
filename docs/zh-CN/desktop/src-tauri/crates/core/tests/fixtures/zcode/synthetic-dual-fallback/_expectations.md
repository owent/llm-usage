# synthetic-dual-fallback 期望（全合成）

<a id="synthetic-dual-fallback-expectations-synthetic"></a>

缺口：response.usage 缺席但 providerMetadata.anthropic.usage 在场（两组用量字段互斥取一，绝不相加）。

详见 tests/zcode_gaps_synthetic.rs 头部人工核算注释。
