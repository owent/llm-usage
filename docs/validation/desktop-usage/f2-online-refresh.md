# F2 可选在线刷新（models.dev）实施验证记录（2026-10-01）

范围：[价格合同 · 在线刷新设计](../../design/desktop-usage/pricing.md#online-refresh)
（任务 5，用户合同：以官方按量付费价为准、不考虑 coding plan；models.dev 必须有缓存；
缓存 TTL 足够长；下载失败用上一次成功结果；无精确匹配回退官方 provider 按量价）。
schema v11、models.dev 转换、抓取/缓存/失败回退、官方提供商回退匹配、命令与界面、
A9 场景测试。不提交、不推送、不部署。

## 实施内容

### 1. 数据源与过滤（core `models_dev.rs`）

- 唯一内置来源 `https://models.dev/api.json`（MIT 社区目录，S18/S19/S24）；
  HTTPS GET（ureq 3.4.2 / rustls），请求不携带任何本地用量、主机身份或密钥。
- 官方提供商判定数据驱动：provider id 是任一模型 `canonical_model_id` 前缀，
  或其 `<id>-cn` 变体；id 含 `-plan`（coding-plan/token-plan 等订阅占位，实测
  cost 全 0）整体排除。模型级 input 与 output 同 0/缺失 ⇒ 跳过；分量 0 ⇒ NULL
  （占位零值不当免费价格）。
- 映射：`cache_write`→`cache_write_5m`（目录无 TTL 分档，`cache_write_1h` 置 NULL）；
  `tiers[type=context]`→`context_threshold_tokens` 行；region=cn（-cn 变体）/global；
  channel=api；currency=USD；美元/百万 token ×10⁴ → 百分之一美分/百万 token
  四舍五入。audio/reasoning 维度不导入；`context_over_200k` 为重复表达不采用。
- 快照 id = `models-dev-YYYYMMDD-hash8`（抓取日期 + 原始内容 FNV-1a），内容寻址幂等。

### 2. 官方提供商回退匹配（schema v11）

- `price_versions.official_vendor`（seed/community 行默认 true，manual 默认 false）；
  `daily_cost_usage.fallback_event_count`。
- 估算匹配链不变；仅在精确链 `no_price_row` 时追加一次回退：official_vendor 行中
  按同 model 匹配，优先用户配置的 region/channel，其余规则（档位/上下文档/生效
  区间）不变；`channel_unknown` 不回退。回退计价事件标记 `official_fallback`，
  日成本与汇总计 `fallback_event_count`，界面以「官方回退」计数标注。
- 预发布合同沿用：不逐版本迁移，v10 库升级走既有「备份→重建→重扫」提示路径。

### 3. 抓取/缓存/失败回退（desktop `price_refresh.rs`）

- 原始响应缓存于 `<数据库目录>/price-cache/models-dev-api.json` + `.meta.json`
  （tmp+rename 原子写；meta 损坏以文件 mtime 重建）。TTL 默认 3 天（设置 1–365
  天），新鲜期内不发网络请求、直接以缓存幂等导入；手动「立即刷新」绕过 TTL。
- 失败回退（A9）：下载或校验失败 ⇒ 回退上一次成功下载的缓存继续导入；无缓存
  时报错并保留既有快照；校验失败的响应不覆盖缓存。
- 命令 `refresh_prices_online(force)`（需费用与在线刷新均启用，后台线程）与
  `price_refresh_status()`；采集结束后 `maybe_auto_refresh` 自动检查（默认关闭
  时不触发）；结果写操作日志 `price_online_refresh`。

### 4. 界面与 i18n

- 设置 → 费用新增「在线价格刷新」面板：启用开关、缓存有效期（天，保存校验
  1–365）、「立即刷新」按钮（轮询 `price_refresh_status` 至完成）、缓存新鲜度
  与最近一次结果展示。
- 趋势页费用面板：按币种行追加官方回退计数（`fallback_event_count`）。
- i18n 新增 13 键 ×10 语言（`cost.refresh.*` 12 + `cost.fallbackCount`）；
  目录总数 329 → 342。

## 真实数据验证（2026-10-01，Windows 11 x64）

- 真实抓取：`GET https://models.dev/api.json` → HTTP 200，5,282,228 字节，
  存于 `build/price-refresh/models-dev-api.json`（gitignored）。
- 转换探针（一次性测试，验后即删）：生产函数 `snapshot_from_models_dev` 对该
  实载产出 473 行 / 24 提供商（官方判定 26，poolside/sarvam 无按量价模型被
  跳过）；抽样 gpt-6-astra（$10/$50/$1/$12.5 + 272K 档 $20/$75）、kimi-k3
  （$3/$15/$0.3）、glm-5.3（$1.4/$4.4/$0.26，cache_write 0 ⇒ NULL）与官方页
  一致；全行 official_vendor=true、currency=USD、channel=api；无 `-plan` 渠道。
- 真实 HTTPS 冒烟（`http_fetch_live_models_dev_smoke`，`--ignored` 手动运行）：
  ureq/rustls 链路实抓成功，响应体转快照非空，退出码 0。

## 测试与命令

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| `cargo test -p llm-usage-core --lib` | 0 | 254 项通过（models_dev 4 项 + 回退匹配 3 项等） |
| `cargo test -p llm-usage-core --test pricing_v29` | 0 | 10 项通过（含 `v29_official_provider_fallback_end_to_end`：精确链缺失时回退官方行、精确链命中不回退、fallback 计数入日成本/汇总） |
| `cargo test -p llm-usage-core` | 0 | 全量 67 个测试目标全绿 |
| `cargo test -p llm-usage-desktop` | 0 | 59 项通过（price_refresh 5 项：A9 失败回退缓存 / 无缓存报错不动快照 / TTL 内不发网络 / 校验失败不覆盖缓存 / 成功写缓存幂等导入） |
| `cargo test -p llm-usage-desktop http_fetch_live -- --ignored` | 0 | 真实 HTTPS 冒烟通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 通过（首轮修 `items_after_test_module`） |
| `cargo fmt --all --check` | 0 | 通过 |
| `npm run verify`（仓库根） | 0 | markdownlint、svelte-check（0 错误 0 警告）、fmt/clippy、Rust 全量测试、vite 构建全通过 |
| `npm run test:browser` | 0 | 浏览器回归通过（见下） |

## 缺口与后置项（如实登记）

1. **社区目录非官方一手**：models.dev 由社区维护，与官方页存在实测偏差案例
   （如 zhipuai 条目挂 docs.z.ai 美元价）；抽样核对一致不等于全量一致。
   快照 ID 含内容哈希，修正随目录更新幂等入库。
2. **`-cn` 变体为美元口径**：目录无币种字段（全美元），中国区条目价是
   docs.z.ai 美元价，未核实是否人民币折算；人民币官方价仍靠仓库种子/手工导入。
3. **代理/企业网未支持**：ureq 直连，未接系统代理设置；失败走缓存回退。
4. **新鲜度以检索时间度量**：目录自身更新频率不可控，TTL 仅约束本机重复下载。
5. 估算行仍不含缓存存储费；usage_observation 区间汇总不参与计价（同 F2 主体缺口）。
6. schema v11 升级使既有本机库走「备份→重建→重扫」提示路径（预发布合同）。
