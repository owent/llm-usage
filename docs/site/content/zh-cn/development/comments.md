---
title: 源码注释与翻译
description: 维护英文源码注释及完整中文索引。
sidebar:
  order: 7
---

源码注释使用英文。中文版本保存在 `docs/source-comments.json`，按仓库路径、稳定标识符和注释顺序索引，
并发布为[源码注释参考](/zh-cn/reference/comments/)。生成的参考页链接到当前源码行。
应用界面字符串、翻译词条、原生测试数据和第三方许可证声明保留原始内容。

## 审阅修改

修改注释前，阅读周围实现和中文参考。英文注释与对应中文条目一起更新。
保留版本、标识符、单位、未知值规则、失败路径及覆盖边界。
解释未核验场景的注释，必须在两种语言中保留这项限定。

索引记录注释类型、英文内容、中文内容和排除注释后的可执行内容摘要。
检查使用 TypeScript 语法、Rust 字符串与注释规则、YAML 具体语法、Python token/docstring，
以及 PowerShell 原生解析器，避免把运行时字符串当成注释。依赖和技术指令仍须有效。

## 验证结果

运行 `npm run check:docs` 检查双语配对和索引一致性，运行 `npm run test:docs` 检查解析回归，
并完成修改文件相关的应用检查。运行 `npm run build:docs` 和 `npm run test:docs:browser`
验证生成的参考页和导航。更新索引仅记录已审阅翻译，不能据此确认运行结果。

审阅源码文件后，在 `build/documentation-site/` 下保存 JSON 数组，按源码顺序为每条注释
提供一个中文字符串，再记录配对：

```powershell
npm run docs:comments:review -- desktop/src-tauri/build.rs --translations build/documentation-site/comments-build.json
```

命令核对英文注释，解析当前源码位置，用已审阅中文字符串更新索引，不改源码文件。
仓库文档或撰写指南在审阅两种语言后，使用 `npm run docs:review -- <英文文件>`。

生成页面位于 `build/documentation-site/content/`。修改源码注释及索引，不编辑生成页。
完整发布流程见[文档维护](/zh-cn/development/documentation/)。
