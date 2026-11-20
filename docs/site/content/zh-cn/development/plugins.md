---
title: 自定义模型名称插件
description: 编写模型别名目录、打包本地插件，并在设置中安装和验证。
sidebar:
  order: 3.5
---

使用带有 **设置 → 模型映射 → 模型名称插件** 的桌面版本。当前包格式支持一份离线模型名称目录。
别名为已采集的本机用量提供显示名称。Agent 采集、价格来源、在线数据源和导出器的规划见[插件设计](/zh-cn/reference/design/plugins/)。

下面的命令使用 PowerShell 7、支持 USTAR 格式的 `tar` 和 `zstd`。先准备一个存放插件源文件的目录。

## 编写目录

把以下示例保存为 UTF-8 无 BOM 的 `aliases.json`。`example-preview` 是合成示例中的模型键。
实际使用时，第一条规则可以选择记录中已有的模型键和 Agent ID。

```json
{
  "schema_version": 1,
  "rule_version": "example-2026-11-16",
  "published_at": "2026-11-16T00:00:00Z",
  "aliases": [
    {
      "agent": "codex",
      "key": "example-preview",
      "display_name": "Example Preview",
      "basis": "catalogue",
      "effective_from": "2026-11-16T00:00:00Z"
    }
  ]
}
```

| 字段 | 填写方法 |
| --- | --- |
| `schema_version` | 填写整数 `1`。 |
| `rule_version` | 为这次目录修订命名。修改规则时更新它，便于在设置中辨认所用的目录。 |
| `published_at` | 使用以 `Z` 结尾的 UTC 时间，最多允许比宿主时钟快五分钟。 |
| `agent`、`key` | 使用记录中的 Agent ID 和原始模型键。两者匹配时忽略大小写，模型键采用应用已有的拼写规范化规则。保留版本后缀和命名空间。 |
| `display_name` | 填写希望在看板和记录明细中显示的名称。 |
| `basis` | 插件的每条规则均填写 `catalogue`。 |
| `effective_from`、`effective_until` | 使用以 `Z` 结尾的 UTC 时间。起点包含在生效区间内，可选的终点排除在外。起点设置在目标用量的发生时间或更早。省略终点可保持开放区间。 |
| `provider_id` | 可选。填写记录中的提供方 ID，可将一条规则限定在该提供方。省略时，规则匹配指定 Agent 和模型键的各提供方。 |
| `source_urls` | 可选。最多填写 8 个无查询参数的公开 HTTPS 链接，用来记录规则的出处。 |

如果看板已显示格式化名称，可以导出明细交换包并查看 `agent`、`model_raw` 和 `provider_id`。选择原始模型键非空的记录。
目录须包含 1–2,000 条规则，文件上限为 1 MiB。[导入与导出](/zh-cn/guide/data/)。[模型映射参考](/zh-cn/reference/design/model-aliases/)。

## 生成清单并打包

在包含 `aliases.json` 的目录执行下面的命令。把 `example.model-names`、`Example`
和 `MIT` 改成自己的插件 ID、发布者和实际许可证，并采用 `1.0.0` 这样的 SemVer 版本。
插件 ID 至少包含两个用点分隔的段，每段使用小写 ASCII 字母、数字和单个连字符。
每段以字母或数字开头和结尾。`builtin.` 和 `llm-usage.` 是保留前缀。

```powershell
$version = '1.0.0'
$catalogHash = (Get-FileHash -LiteralPath aliases.json -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = [ordered]@{
    format = 'llm-usage-plugin/1'
    api_major = 1
    id = 'example.model-names'
    version = $version
    name = 'Example model names'
    publisher = 'Example'
    license = 'MIT'
    contribution = [ordered]@{
        id = 'names'
        kind = 'model_aliases'
        entry = 'aliases.json'
        sha256 = $catalogHash
    }
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath manifest.json -Encoding utf8NoBOM

$archive = "example-model-names-$version.tar"
tar --format ustar -cf $archive manifest.json aliases.json
if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
zstd -1 -f $archive -o "${archive}.zst"
if ($LASTEXITCODE -ne 0) { throw 'zstd failed' }
```

命令生成 `example-model-names-1.0.0.tar.zst`。归档根目录恰好包含两个普通文件
`manifest.json` 和 `aliases.json`。压缩包上限为 2 MiB，清单上限为 16 KiB。
打包时只归档这两个文件。命令按目录文件的最终字节计算摘要，任何编辑后都要重新执行，包括换行符变化。

## 安装并验证

1. 在 **设置 → 模型映射 → 模型名称插件**，点击 **选择插件包** 并选择生成的 `.tar.zst`。
2. 核对预览中的 ID、版本和规则数量。本地未签名包会显示 **发布者未经验证**。
   预览中的摘要标识清单字节，清单中包含目录摘要。
3. 点击 **安装并保持禁用**，再点击 **启用此版本**。
4. 在 **详情** 中选取生效区间内相应 Agent 和模型的用量，检查显示名称。看板汇总的时间范围应全部由同一名称覆盖。
   用量继续使用原始模型键标识，费用按记录中的计价身份计算。

## 更新、回滚或移除

为同一插件发布另一版本时沿用插件 ID。任一输入文件发生变化时，在打包命令中改用新的 `$version`。
安装新包后启用其版本。同一插件 ID 只启用一个版本。需要回滚时，启用仍已安装的旧版本。
**禁用**会撤销显示名称。**移除**会把该版本从已安装列表移出，目录保留在停用的历史记录中。
重复安装相同的清单和目录是幂等操作。

## 排查打包失败或名称未生效

| 现象或错误码 | 检查项 |
| --- | --- |
| `unsupported_manifest` | 检查 `format` 是否为 `llm-usage-plugin/1`、`api_major` 是否为数字 `1`，以及 `contribution.kind` 和 `contribution.entry` 是否为 `model_aliases` 和 `aliases.json`。 |
| `invalid_manifest` 或 `model_alias_json` | 检查无 BOM 的 UTF-8 编码、严格 JSON、必需字段、插件 ID 和版本。字段名须与示例一致。 |
| `asset_digest_mismatch` | 重新计算最终 `aliases.json` 字节的摘要，并按打包命令重建归档。 |
| `invalid_archive_member`、`duplicate_archive_member`、`missing_manifest` 或 `missing_aliases` | 使用精确的小写文件名重新打包，把文件直接放在归档根目录，排除上层目录、链接和重复条目。 |
| `invalid_plugin_alias_basis` 或 `model_alias_rule` | 将 `basis` 设为 `catalogue`。检查区间顺序以及可选的提供方和出处链接。 |
| `model_alias_future_publication` | 检查计算机时钟及 `published_at` 的 UTC 值。 |
| `package_size_limit` | 检查压缩包及各源文件的大小。包内只保留声明的两个文件。 |
| `package_version_conflict` | 保留该 ID 和版本对应的原包，为修改后的包设置新版本并重新打包。 |
| 已启用但名称未生效 | 核对记录中的 Agent、模型和提供方，再缩小日期范围。适用的内置及已导入映射优先。覆盖缺口或插件名称冲突时可能显示原始模型键。只复制数据库时，可能还需在当前机器重新启用。 |

宿主行为和限制见[插件包与存储说明](/zh-cn/reference/design/plugins/#本地模型名称包)。
