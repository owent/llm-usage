# synthetic-runtime-store expectations (synthetic)

<a id="synthetic-runtime-store-期望全合成"></a>

The file is agents/main/agent/openclaw-agent.sqlite, using placeholder tables
synthetic_table_rows/synthetic_table_events because documentation did not identify native table names.

Manually calculated expectations:

- Discover under default root `<home>/.openclaw`; instance root=agents/main, one instance per Agent.
- The documented path identifies the Agent, but undocumented tables prevent schema validation.
  Return unknown_format with reason "docs do not name any table/column; pending a real sample".
  The readable database's two user tables appear in the reason.
- No usage_events/source_aggregates: do not read guessed tables/fields or generate zero usage.
- Repeat scans preserve state/counts and produce no data.
