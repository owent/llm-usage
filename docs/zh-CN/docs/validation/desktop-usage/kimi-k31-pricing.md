# Kimi K3.1 Pro 计价修复

版本为 0.3.4 工作区。官方资料于 2026-10-24 核验，本地验证持续至 2026-10-25
（Asia/Shanghai）。保留已有依赖、数据源和价格维护改动。本次未提交、发布或替换已安装应用。

## 原因与修复

模型识别原先支持 `k3`、`k3-256k`，缺少 `k3.1-pro`；已有离线价格快照也缺少
`kimi-k3.1-pro`。存在已知用量时，无 provider 的裸模型名返回 `no_provider`，
未配置渠道的自定义 provider 返回 `channel_unknown`，完整的 `kimi-k3.1-pro`
型号返回 `no_price_row`。只补别名或只补价格，都无法让裸服务 ID 完成计价。

`model_names.rs` 现在将两个明确的 Kimi Code ID 分别映射至对应 API ID：
`k3.1-pro` → `kimi-k3.1-pro`，`k3.1-pro-highspeed` → `kimi-k3.1-pro-highspeed`。
沿用已有 provider 前缀和拼写比较规则，保留未知命名空间、后缀及其他版本的区别，
不修改原始模型名和 provider。

独立的 [种子价格快照](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-24-kimi-k3.1.json)
新增四条标准 API 价目。启动时在事务内向已有数据库幂等导入，不替换已有不可变快照。
价格从核验日起生效，不推断发布当日价格。当前参考价可以用于较早保留的用量，
导入和查询不会重算已保存的发生时估算。

## 官方价格

型号对应关系来自 [Kimi Code 模型表](https://www.kimi.com/code/docs/en/kimi-code/models.html)。
费率来自 [国内 API 价格页](https://platform.kimi.com/docs/pricing/chat) 和
[国际 API 价格页](https://platform.kimi.ai/docs/pricing/chat)。下表均为每 1,000,000 token
的原币种价格；国际价格不含适用税费。

| 型号 | 币种 | 输入 | 缓存命中 | 输出 | 写入 5 分钟 | 写入 1 小时 |
| --- | --- | --- | --- | --- | --- | --- |
| K3.1 Pro | CNY | 13 | 1.30 | 68 | 13 | 26 |
| K3.1 Pro | USD | 2 | 0.20 | 10 | 2 | 4 |
| K3.1 Pro Highspeed | CNY | 26 | 1.30 | 136 | 26 | 52 |
| K3.1 Pro Highspeed | USD | 4 | 0.20 | 20 | 4 | 8 |

官方 Markdown 导出和网页文本提取均遗漏了动态价格表。HTML 响应包含编译后的
`DocTable` 列定义和数据行，本次一起核对两者，未执行下载的代码。公开页面副本位于
已忽略的 `build/kimi-k31-pricing/`，SHA-256 摘要为：

- 国内 HTML：`fd01d7db4446829244152899b82341e4ed625934a56e49904bb430d70c5074ea`。
- 国际 HTML：`8b5227c3516c83d3492b2e97284bcfed40bb0374609ec2fe4e2bba22e8d5d407`。

缓存存储没有已核验费率，保持未知。缺少缓存写入 TTL 时仍不计该项费用，除非已有
provider TTL 设置提供明确值。精确配置的价格优先；否则沿用已有参考选择规则，
在没有匹配渠道偏好时优先选国际 USD。国内价格保留 CNY。Kimi Code 订阅用量显示
API 价格参考，不将其解释为订阅实际支付金额。

## 本地验证

Windows x64，Node 24.21.0，Cargo/Rust 工具链 1.99.1；命令在仓库根执行。
每次测试都在 `build/kimi-k31-pricing/` 下创建新的 UUID 目录作为 `TEMP` 和 `TMP`，
将合成测试数据库与用户数据、之前的运行隔离。

修正最初新增测试与现有响应类型不符的编译错误后，四项新测试均在原实现上失败
（退出码 101），复现缺少别名、价格及启动导入。修复后四项全部通过（退出码 0）。

定向回归命令通过 48 项测试（退出码 0）：

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked --test pricing_kimi_k31 --test pricing_v29 --test pricing_archives --test pricing_substitutes --test model_reference --test dashboard_repair
```

覆盖裸模型名、带前缀和自定义 provider 的 ID，两种币种及速度版本，缺少输入时的
已知分项计价，未知 TTL、明确 5 分钟/1 小时设置，精确价格优先，拒绝未知型号，
核验日期边界，旧库重新打开和重复导入，以及旧快照元数据、历史费用保持不变。
看板汇总、模型金额和单价、日曲线、小时选区的参考费用一致。已归档日汇总与明细
共存及明细删除后均保留相同费用。既有周/月归档、小额汇总和 K2.8 替代价格测试也通过。

其他检查退出码均为 0：

| 命令 | 结果 |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked --lib pricing` | 13 项计价单元测试通过 |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --all-targets --locked -- -D warnings` | 通过 |
| `npm run fmt:check` | 通过 |
| `npm run lint:md` | 473 个 Markdown 文件，无问题 |
| `npm run check:docs` | 210 对仓库文档、21 对指南；Astro 无错误、警告或提示 |
| `git diff --check` | 通过；另行审阅新增文件 |

最初沙箱无法启动 PowerShell（`CreateProcessAsUserW failed: 5`），通过平台允许的
提权执行完成检查。这属于执行环境错误，不是产品测试失败。这些本地检查不代表
已安装 GUI、供应商账单、远程 CI 或其他平台验收。本次未修改真实 Agent 源文件或
用户统计数据库，也未发起模型 API 调用。
