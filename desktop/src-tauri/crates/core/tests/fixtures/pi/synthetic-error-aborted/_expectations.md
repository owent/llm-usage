# synthetic-error-aborted._expectations.md (synthetic)

<a id="synthetic-error-aborted_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** Test persistence of
stopReason=error/aborted as error_status, using Pi session JSONL v3.

<a id="场景与期望"></a>

## Scenario and expectations

Three lines: session syn-sess-err; assistant syn-e-1, stopReason=error, usage 10/20/0/0/30;
assistant syn-e-2, stopReason=aborted, usage 5/5/5/0/15.

- Two primary events, error_status="error" and "aborted" respectively.
- 2026-01-05 summary: call_count=2, input_total_known=20 (10+5+5), cache_read_known=5,
  cache_write_known=0, output_total_known=25 (20+5), total_tokens_known=45 (30+15).
- Both have all five usage fields with matching totalTokens; no diagnostics.
