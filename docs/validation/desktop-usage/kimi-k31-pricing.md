# Kimi K3.1 Pro pricing repair

Version 0.3.4 working tree. Official sources were checked on 2026-10-24; local verification
continued on 2026-10-25 (Asia/Shanghai). Existing dependency, source and price maintenance
changes were retained. No commit, release or installed application replacement was performed.

## Cause and repair

The model resolver recognized `k3` and `k3-256k`, but not `k3.1-pro`. Existing offline
snapshots also lacked `kimi-k3.1-pro`. With known usage, a bare model without a provider
returned `no_provider`; a custom provider without a configured channel returned
`channel_unknown`; the full `kimi-k3.1-pro` identity returned `no_price_row`.
Adding only an alias or only a price would leave the bare serving ID unpriced.

`model_names.rs` now maps the two explicit Kimi Code IDs to their corresponding API IDs:
`k3.1-pro` → `kimi-k3.1-pro` and `k3.1-pro-highspeed` → `kimi-k3.1-pro-highspeed`.
Existing provider-prefix and spelling rules apply. Unknown namespaces, suffixes and
other versions remain distinct. Raw model/provider values are preserved.

The independent [seed snapshot](../../../desktop/src-tauri/crates/core/prices/seed-2026-10-24-kimi-k3.1.json)
adds four standard API price rows. Startup imports it transactionally and idempotently into
existing databases without replacing earlier immutable snapshots. Prices start at verification
date, not an inferred launch date. Current references can price earlier retained usage;
saved occurrence-time estimates are not recomputed by the import or query.

## Official prices

Model identity comes from the [Kimi Code model table](https://www.kimi.com/code/docs/en/kimi-code/models.html).
Rates come from the [CN API price page](https://platform.kimi.com/docs/pricing/chat) and
[global API price page](https://platform.kimi.ai/docs/pricing/chat). Each amount below is per
1,000,000 tokens in the stated currency. Global prices exclude applicable taxes.

| Model | Currency | Input | Cache hit | Output | Write 5m | Write 1h |
| --- | --- | --- | --- | --- | --- | --- |
| K3.1 Pro | CNY | 13 | 1.30 | 68 | 13 | 26 |
| K3.1 Pro | USD | 2 | 0.20 | 10 | 2 | 4 |
| K3.1 Pro Highspeed | CNY | 26 | 1.30 | 136 | 26 | 52 |
| K3.1 Pro Highspeed | USD | 4 | 0.20 | 20 | 4 | 8 |

The official Markdown exports and extracted page text omitted the dynamic price tables.
The HTML responses contained the compiled `DocTable` column definitions and rows; both
were inspected together without executing downloaded code. Local public-page copies are
under ignored `build/kimi-k31-pricing/`. Their SHA-256 digests are:

- CN HTML: `fd01d7db4446829244152899b82341e4ed625934a56e49904bb430d70c5074ea`.
- Global HTML: `8b5227c3516c83d3492b2e97284bcfed40bb0374609ec2fe4e2bba22e8d5d407`.

Cache storage has no verified rate and stays unknown. Missing cache-write TTL remains
unpriced unless the existing provider TTL setting supplies it. Exact configured prices win;
otherwise the existing reference selection prefers global USD unless a matching channel is
selected. CN prices remain CNY. Kimi Code subscription usage receives an API reference,
not a claim about subscription payments.

## Local validation

Windows x64, Node 24.21.0, Cargo/Rust toolchain 1.99.1; commands ran from the repository
root. Every test invocation used a new UUID directory under `build/kimi-k31-pricing/`
for `TEMP` and `TMP`, keeping synthetic databases separate from user data and earlier runs.

After correcting initial test compilation errors against the existing response types,
the four new tests failed on the original implementation (exit 101), reproducing missing
alias/rates and absent startup import. All four passed after the repair (exit 0).

The focused regression command passed 48 tests (exit 0):

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked --test pricing_kimi_k31 --test pricing_v29 --test pricing_archives --test pricing_substitutes --test model_reference --test dashboard_repair
```

Coverage includes bare/prefixed/custom-provider IDs, both currencies and speed variants,
known components with missing input, unknown TTL, explicit 5m/1h settings, exact-price
priority, unknown model rejection, verification-date boundaries, old-database reopen and
repeat import, and unchanged old snapshot metadata/historical costs. Dashboard summary,
model amounts/unit prices, daily curves and selected-hour references agree. Sealed daily
summaries with coexisting details and after detail removal retain the same cost.
Existing weekly/monthly archive, small-value aggregation and K2.8 substitution tests also pass.

Additional checks all exited 0:

| Command | Result |
| --- | --- |
| `cargo test --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --locked --lib pricing` | 13 pricing unit tests passed |
| `cargo clippy --manifest-path desktop/src-tauri/Cargo.toml -p llm-usage-core --all-targets --locked -- -D warnings` | Passed |
| `npm run fmt:check` | Passed |
| `npm run lint:md` | 473 Markdown files, no issues |
| `npm run check:docs` | 210 repository translation pairs and 21 guide pairs; Astro: no errors, warnings or hints |
| `git diff --check` | Passed; new files also reviewed separately |

The initial sandbox could not launch PowerShell (`CreateProcessAsUserW failed: 5`);
the permitted escalated execution completed the checks. This was an execution-environment
failure, not a product test failure. These local checks do not establish installed-GUI,
provider-bill, remote CI or other-platform acceptance. No real Agent sources or user
statistics databases were modified, and no model API call was made.
