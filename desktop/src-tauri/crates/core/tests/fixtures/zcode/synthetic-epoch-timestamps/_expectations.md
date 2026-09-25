# synthetic-epoch-timestamps 期望（全合成）

防护：completedAt 为数字（毫秒 1800000000000 / 秒 1800000000）⇒ 统一折算毫秒（<1e11
  视为秒）；model-io 实读为 ISO 字符串，本目录为防御性合成覆盖。

详见 tests/zcode_gaps_synthetic.rs 头部人工核算注释。
