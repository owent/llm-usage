# synthetic-detect-supported._expectations.md (SYNTHETIC)

<a id="synthetic-detect-supported_expectationsmdsynthetic"></a>

**All samples in this directory are synthetic, rather than extracted native sessions.**
Structure follows oh-my-pi 18.2.7 session JSONL: title first, session header second, and
common entry fields {type,id,parentId,timestamp}.

<a id="场景与期望"></a>

## Scenario and expectations

- Five lines: title, session (version=3, id=syn-omp-detect), model_change (native OMP
  model="provider/model" shape for structured attribution), user message, and assistant
  message with usage and OMP-specific duration/ttft in floating-point milliseconds.
- Detection: first-line title and version=3 session header within the first four lines
  give Supported(format=omp-session-jsonl, format_version="3", basis=known_version).
  The V30 registry maps registered version 3 to KnownVersion.
- Scan: one complete file, records_seen=5, one primary event, added=1.
- Assistant usage: input=100, output=10, cacheRead=0, cacheWrite=0, totalTokens=110.
  Mapping gives input_uncached=100, input_total=100 (derived: 100+0+0), total_tokens=110
  (derived: input_total+output), and source_total=110. Round duration 100.4→100 and ttft 50.4→50.
- Summary (2026-01-05 UTC): call_count=1, input_total_known=100,
  cache_read_known=Some(0), output_total_known=10, total_tokens_known=110.
