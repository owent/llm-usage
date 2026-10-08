# synthetic-error-aborted._expectations.md (SYNTHETIC)

<a id="synthetic-error-aborted_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
stopReason=error/aborted maps to error_status. Local native verification found usage on
all 147 error/aborted records; aborted records had no duration/ttft fields, matching the
native k3 sample.

<a id="场景与期望"></a>

## Scenario and expectations

- Four lines: title, session (version=3, id=syn-omp-err), assistant syn-e-1
  (stopReason=error, usage 15/20/5/0/40, duration=100.4→100, absent ttft=None), and
  assistant syn-e-2 (stopReason=aborted, usage 5/5/0/0/10, both duration and ttft absent).
- Scan: one complete file, two primary events, added=2; error_status is error and aborted.
- Summary (2026-01-05 UTC): call_count=2,
  input_total_known=25 (derived: (15+5+0)+(5+0+0)), cache_read_known=5,
  cache_write_known=0, output_total_known=25, total_tokens_known=50.
