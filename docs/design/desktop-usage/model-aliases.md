# Versioned model name mappings

This feature updates model display mappings independently of prices and application releases.
Raw models, providers, usage, attribution, grouping/filter keys and stored estimates remain unchanged.
The current Qoder `qmodel_latest` → `Qwen4-Max-Preview` user-authorized fallback is retained.
A catalogue name is not proof of the model actually served after a provider-side fallback.

## Sources and refresh

- Bundle the existing display override for offline startup. Its `user_override` basis remains explicit.
- Import a user-selected local JSON catalogue after validation. Do not scan arbitrary Agent files or
  extract names from prompts, answers, generic logs or an installed version.
- Optional online refresh defaults off, independently of cost estimation. The fixed public source is
  `https://llm-usage.atframe.work/models/aliases.json`; an exact repository raw-file fallback may be
  used for that same catalogue. No account credentials, usage, source paths, host IDs or query models
  enter requests. Do not call account/billing/model-generation APIs.
- Refresh at GUI startup and after completed collection when stale. Cache for three days by default,
  configurable from 1 to 365 days; manual refresh bypasses freshness. Bound HTTPS time and response size,
  reject redirects and use only the two fixed public URLs. A failure retains the last valid catalogue
  and reports an error without blocking collection. An offline/manual catalogue remains usable.

The published feed is an ordinary documentation static resource, maintained with the repository.
The new endpoint becomes available after the corresponding documentation deployment. Preparing the
resource or passing a synthetic download test does not establish that public deployment has occurred.
The exact fallback is
`https://raw.githubusercontent.com/owent/llm-usage/main/docs/site/public/models/aliases.json`.
The project maintains this feed after checking sources. It is not automatic identification of a
provider's hidden routing or a live official Qoder key-to-model API.

## Catalogue format

JSON uses `schema_version: 1`, a nonempty `rule_version`, UTC `published_at`, and bounded `aliases`.
Each entry has an exact Agent ID, raw `key`, optional exact `provider_id`, `display_name`,
`effective_from` and optional exclusive `effective_until`, plus public `source_urls` and `basis`.
Only separator/case spelling normalization is allowed. Unknown namespaces, suffixes and releases
remain distinct. Reject invalid timestamps, empty scopes, control characters, excessive lengths,
duplicate rules, unknown schema fields and invalid intervals. Error replies never echo JSON bodies.

```json
{
  "schema_version": 1,
  "rule_version": "example-2026-11-16",
  "published_at": "2026-11-16T00:00:00Z",
  "aliases": [{
    "agent": "qoder",
    "key": "qmodel_latest",
    "display_name": "Qwen4-Max-Preview",
    "effective_from": "2026-11-16T00:00:00Z",
    "basis": "user_override",
    "source_urls": []
  }]
}
```

Accepted bases are `user_override`, `catalogue` and `native_snapshot`; they record the claimed
basis, not independent certification. Bound catalogues to 1 MiB/2,000 rules, candidate history to
20,000 rules/4,096 versions and stored metadata to 32 MiB. Exceeding a bound retains the old state.

The catalogue supplies mappings for declared scopes only. A provider-constrained rule requires that
provider on the record; an installed client or database-wide version cannot identify other records.
The initial format does not certify client-version/host-specific catalogues. Such scopes require a
separate verified format extension; unsupported fields are rejected rather than ignored.

## Persistence and conflict handling

Store validated catalogue candidates and the latest successful raw response/hash in an application
metadata setting, using the existing writer and one transaction. Do not change database schema version
or rebuild databases. Download outside the writer lock. A background worker cannot replace a newer
catalogue after a manual import, settings change, or another process's update.

Keep every candidate with its source/version/hash and effective interval. Repeating the same content
is idempotent; the same version with different content is invalid. Older published feeds cannot replace
the successful cache. A later effective start supersedes an earlier open interval for future display;
it does not rewrite the earlier candidate. Different names with the same start/scope remain a conflict,
including after later downloads. Expiring a newer interval must not revive an older open mapping.
SHA-256 identifies cached content; it is not a publisher signature. HTTPS and repository ownership are
the trust boundary. Corrupt cache/state is reported and never silently treated as verified data.

## Queries and UI

Resolve a record's own canonical model first when it is already persisted; this change does not add
unverified native readers or fill absent canonical fields. Otherwise use exact Agent/provider scope and
the usage/query time range. Different providers and same-named routes stay separate. Conflicting or
uncovered intervals retain the raw key. Aggregates spanning different names also retain the raw key;
do not assign their tokens or calls to one of those model releases.
An exact provider rule takes precedence over an Agent-wide rule. Within that scope, the latest
effective start applies. Hourly aggregate labels conservatively require coverage of the complete
local day, including repeated DST hours; record details use their exact occurrence time.

Summary/chart label decisions use the same retained source partitions, filters and read snapshot as
their usage. Acceleration must not erase the Agent scope needed for a label. Metadata generation is
part of summary cache keys and UI refresh detection, without changing the usage revision. Details use
their own record time. Current filter labels may use the present catalogue, but their values remain raw.
All table titles keep the original key. Prices/reference-model selection do not consume display mappings.

Settings expose online enablement, TTL, refresh status, refresh-now and local catalogue import.
Enabling online metadata needs no cost-estimation switch. Existing settings deserialize with refresh
disabled. Keep controls localized, preserve autosave drafts and retain valid metadata after failures.
Disable online refresh to stop downloads. To change a mapping, import a new version with an explicit
effective interval; preserve existing candidates and usage databases. Older application code can ignore
this metadata setting without a schema migration.

## Required checks

Test parser bounds, duplicate/version conflicts, provider/Agent scope, interval edges and gaps, future
entries, mixed aggregate ranges, closed-interval non-revival, repeated imports, cache corruption,
TTL/failure fallback, transaction rollback and stale worker/cross-process arbitration. Verify old
databases without clearing them, raw exports/prices unchanged, accelerated/daily/archived/hourly queries,
and live UI labels after metadata-only updates. Separate synthetic HTTP/IPC, native executable and
public feed checks. Temporary artifacts belong under root `build/model-aliases-20261116/`.
