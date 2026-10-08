# real-subagent expectations (anonymized native sample, local ZCode 3.14.3)

<a id="real-subagent-期望真实脱敏样本本机-zcode-3143"></a>

This is **native** test data, extracted read-only locally on 2026-09-25 from the same
run as real-main-session. anon numbers share that run's stable mapping. Source:
`cli/rollout/model-io-sess_subagent_agent_<id>.jsonl`, first four records of seven
original lines. Version verification is the same 3.14.3 source as real-main-session.

<a id="逐条数值人工核对"></a>

## Individual values (manually checked)

| line | Request (req) | AI SDK in/out/total/cr/cw | anthropic in/out/cr | Consistency |
| --- | --- | --- | --- | --- |
| 1 | anon-20 | 145225/2984/148209/142656/0 | 2569/2984/142656 | 2569+142656=145225 ✓ |
| 2 | anon-23 | 149108/607/149715/145216/0 | 3892/607/145216 | 3892+145216=149108 ✓ |
| 3 | anon-25 | 150451/754/151205/149056/0 | 1395/754/149056 | 1395+149056=150451 ✓ |
| 4 | anon-27 | 151581/249/151830/150400/0 | 1181/249/150400 | 1181+150400=151581 ✓ |

All have querySource=subagent (sub_agent), attempt=1, turn_anon-21,
sessionId=sess_subagent_agent_anon-19, model=GLM-5.3 / account:bigmodel-indivi.
Durations: 44908/10810/9776/6617 ms.

<a id="期望单文件"></a>

## Single-file expectations

- Four events, identity zcode:{requestId}:1, call_category=sub_agent,
  session_id=sess_subagent_agent_anon-19, schema_version="3.14.3".
- Primary AI SDK mapping: input_uncached=2569/3892/1395/1181 (derived), sum=9037.
- Summary (UTC 2026-09-25): call_count=4, input_total_known=596,365,
  output_total_known=4,594, cache_read_known=587,328, cache_write_known=Some(0),
  total_tokens_known=600,959. Zero diagnostics.

<a id="合并期望与-real-main-session-放同一-clirollout-根"></a>

## Combined expectations (with real-main-session under one cli/rollout root)

- call_count=8 (four primary, four sub_agent), input_total_known=2,163,849,
  output_total_known=5,768, cache_read_known=2,153,024, cache_write_known=Some(0),
  total_tokens_known=2,169,617, input_uncached sum=10,825 (=2,163,849−2,153,024−0).
- Anthropic sums input_tokens=10,825, cache_read=2,153,024; cache_creation absent.
  These agree with exclusive AI SDK fields: 10,825+2,153,024=2,163,849.
  Select one representation; do not mix or add both.
