# 验证记录模板

<a id="validation-record-template"></a>

复制本模板创建 `<阶段>-<主题>.md`（如 `m0-windows-baseline.md`）。所有字段按实际填写；
未执行的项目保留并标注"未执行"及原因，不删除、不预填结果。

<a id="metadata"></a>

## 元信息

| 项目 | 内容 |
| --- | --- |
| 日期 | YYYY-MM-DD |
| 执行环境 | OS 版本/架构、CPU、内存、运行时版本（Node/Rust/WebView 等） |
| 代码 revision | commit 或工作树状态摘要 |
| 对应设计要求 | 对应设计文档章节与 V 编号 |

<a id="commands-and-results"></a>

## 命令与结果

每条命令记录：cwd、完整命令、退出码、耗时、关键输出摘要。

| # | 命令（cwd） | 退出码 | 结果摘要 |
| --- | --- | --- | --- |
| 1 | | | |

<a id="locked-versions"></a>

## 锁定版本

| 依赖 | 锁定版本 | 实际解析（锁文件值） | 许可证 | 来源 |
| --- | --- | --- | --- | --- |

<a id="measurements-and-tests"></a>

## 测量与测试

| 指标/用例 | 目标 | 实测 | 方法与样本 | 结论 |
| --- | --- | --- | --- | --- |

<a id="failures-and-unexecuted-items"></a>

## 失败与未执行项

| 项 | 状态 | 原因 | 后续条件 |
| --- | --- | --- | --- |

<a id="证据文件"></a>

<a id="validation-artifacts"></a>

## 验证产物

列出产物路径（fixtures、构建制品、日志），注明存放位置与脱敏方法。
