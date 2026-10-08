# OpenCode 1.18.34 explicit-title comparison

<a id="opencode-11834-明确标题对照"></a>

On 2026-10-05, the official CLI at source revision
aec0b9a6d8898f68f923aaf08b7306d931fd9d76 ran in isolated rootless Podman with networking
disabled, a local CPU model and fresh HOME. The only change from the default sample was
an explicit title through the actual `run --title` option.
[Official ensureTitle](https://github.com/anomalyco/opencode/blob/aec0b9a6d8898f68f923aaf08b7306d931fd9d76/packages/opencode/src/session/prompt.ts)
skips generation for non-default titles; only one API usage record was observed.

API input=298, output=1, total=299, cache read=0. CLI, part, assistant message and session
all report input=298, output=1, reasoning=0, cache read/write=0 and total=299.
The application stores one event with input_total=298 and total=299; repeat scanning
changes no events, revisions or summaries. The three tables' DDL and usage fields are
unchanged. Bodies are removed; IDs, paths and titles are anonymized, with timestamps
shifted within the same day.

This comparison leaves the default-title coverage gap unresolved. It verifies neither
cloud providers, other versions, positive costs, cache write nor reasoning values.
The version remains latest_fallback; reasons and the complete collection procedure are
in the [container record](../../../../../../../../docs/validation/desktop-usage/container-sources.md).
