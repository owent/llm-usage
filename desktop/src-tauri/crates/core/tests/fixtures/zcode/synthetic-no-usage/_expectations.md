# synthetic-no-usage 期望（全合成）

缺口：末条记录 response.finishReason=null 且无 usage/providerMetadata（在途尾部）⇒
  正常形状，不产事件、不失败。

详见 tests/zcode_gaps_synthetic.rs 头部人工核算注释。
