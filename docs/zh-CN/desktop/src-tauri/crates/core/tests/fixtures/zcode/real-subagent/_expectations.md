# real-subagent 期望（真实脱敏样本，本机 ZCode 3.14.3）

<a id="real-subagent-expectations-anonymized-native-sample-local-zcode-3143"></a>

本目录为**真实** fixture（2026-09-25 本机只读提取；与 real-main-session 同次运行，
anon 编号在同次映射内稳定）。来源：`cli/rollout/model-io-sess_subagent_agent_<id>.jsonl`
头部 4 条（提取时源文件共 7 行）。版本核验来源同 real-main-session（3.14.3）。

<a id="individual-values-manually-checked"></a>

## 逐条数值（人工核对）

| line | 请求（req） | AI SDK in/out/total/cr/cw | anthropic in/out/cr | 一致性 |
| --- | --- | --- | --- | --- |
| 1 | anon-20 | 145225/2984/148209/142656/0 | 2569/2984/142656 | 2569+142656=145225 ✓ |
| 2 | anon-23 | 149108/607/149715/145216/0 | 3892/607/145216 | 3892+145216=149108 ✓ |
| 3 | anon-25 | 150451/754/151205/149056/0 | 1395/754/149056 | 1395+149056=150451 ✓ |
| 4 | anon-27 | 151581/249/151830/150400/0 | 1181/249/150400 | 1181+150400=151581 ✓ |

全部 `querySource=subagent`（⇒ sub_agent 分类）、`attempt=1`、同一 `turn_anon-21`、
`sessionId=sess_subagent_agent_anon-19`、model=GLM-5.3 / account:bigmodel-indivi
duration 44908/10810/9776/6617 ms。

<a id="single-file-expectations"></a>

## 期望（单文件）

- 事件 4 条；身份 `zcode:{requestId}:1`；call_category=sub_agent；
  session_id=sess_subagent_agent_anon-19；schema_version="3.14.3"。

- 映射同主调用计算方法（AI SDK）：input_uncached=2569/3892/1395/1181（derived，和 9037）。

- 汇总（UTC 2026-09-25，单文件）：call_count=4；input_total_known=596,365；
  output_total_known=4,594；cache_read_known=587,328；cache_write_known=Some(0)；
  total_tokens_known=600,959。

- 诊断 0。

<a id="combined-expectations-with-real-main-session-under-one-clirollout-root"></a>

## 合并期望（与 real-main-session 放同一 cli/rollout 根）

- call_count=8（4 primary + 4 sub_agent）；input_total_known=2,163,849；
  output_total_known=5,768；cache_read_known=2,153,024；cache_write_known=Some(0)；
  total_tokens_known=2,169,617；input_uncached 合计 10,825（=2,163,849−2,153,024−0）。

- anthropic 侧合计 input_tokens=10,825、cache_read=2,153,024、cache_creation 缺席
  ⇒ 与 AI SDK 互斥字段一致（10,825+2,153,024=2,163,849），两组字段不混算、不双计。
