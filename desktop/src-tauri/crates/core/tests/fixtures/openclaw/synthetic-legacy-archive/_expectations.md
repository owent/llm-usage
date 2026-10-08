# synthetic-legacy-archive expectations (synthetic)

<a id="synthetic-legacy-archive-期望全合成"></a>

Legacy JSONL under agents/main/sessions/ uses placeholder normalized usage fields from
documentation: input_tokens/output_tokens, prompt_tokens/completion_tokens aliases and
cacheRead. sessions/sessions.json is another legacy migration input.

Manually calculated expectations:

- Discover both files with instance root=agents/main.
- Treat JSONL as **migration/offline maintenance input**. Official documentation says Gateway
  startup does not import it and openclaw doctor --fix must migrate it; entry schema is undocumented.
  Return unknown_format with a pending-verification reason. sessions.json likewise returns
  unknown_format, identifying the doctor migration requirement.
- No usage_events/source_aggregates. Synthetic values such as 1200+300/60+15 **do not count**;
  native schema is unverified, with no guessed fields or zero usage.
- Repeat scans add no data.
