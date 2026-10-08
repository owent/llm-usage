# Hermes Agent test samples

<a id="hermes-agent-fixtures"></a>

<a id="hermes-agent-合成-fixtures文档级证据待真实样本"></a>

synthetic-* uses syn- IDs to cover auxiliary usage, cross-day intervals, backfills and
numeric boundaries. real-0.21.5 comes from the official image's actual CLI/public session
continuation with a local model in isolated Podman. Retain native DDL/permitted fields,
API/CLI usage comparisons and version/digest references; exclude bodies, configuration and credentials.

<a id="源码依据a24固定-commit-ef70b3661cbfcf57e583008ad91dd04d8ba46070"></a>

<a id="源码级证据a24固定-commit-ef70b3661cbfcf57e583008ad91dd04d8ba46070"></a>

## Source references (A24, fixed commit ef70b3661cbfcf57e583008ad91dd04d8ba46070)

- SCHEMA_SQL: sessions/session_model_usage, 18 columns and six-column primary key:
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_common.py>
- Cumulative counters: incremental/absolute; record_auxiliary_usage writes task keys
  only; first_seen on insertion, last_seen advances:
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_usage.py>
- v20 backfill/v22 primary-key migration:
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_state_schema.py>
- Path order: HERMES_HOME → %LOCALAPPDATA%/hermes → ~/.hermes; named profiles:
  <https://github.com/NousResearch/hermes-agent/blob/ef70b3661cbfcf57e583008ad91dd04d8ba46070/hermes_constants.py>
- [Official storage documentation](https://hermes-agent.nousresearch.com/docs/developer-guide/session-storage/).

session_model_usage DDL uses exact fixed-source column names/primary key. sessions keeps
only columns read by the adapter for time fallback and lineage, with source-matching
definitions. schema_version=30 matches fixed SCHEMA_VERSION.

<a id="场景"></a>

## Scenarios

| Directory | Coverage |
| --- | --- |
| synthetic-basic-cumulative | Single-day cumulative row: five counters, api_call_count, first/last_seen |
| synthetic-aux-task-mutex | Main-model 100 plus separate task auxiliary 20 gives 120, not 220 |
| synthetic-cross-day | One aggregate spanning two days; neither split into three calls nor assigned to one day |
| synthetic-v20-backfill-compression | v20 backfill with NULL first/last_seen using session times; compressed child does not duplicate parent usage |
| real-0.21.5 | Two native streaming calls: uncached input 849, cache read 812, output 4, source-reported call subtotal 2 |

Each _expectations.md has manually calculated values. Version 0.21.5 commit
f97608f178d1ffeca59860195ab7da295f7c8e5f and A24 agent/usage_pricing.py confirm uncached
input and reasoning as an output subset. Missing fields/initialized zeros are stored
as zero, so default zeros remain unknown. Derive input/complete total only with every
required component known. Real API total input=1661 and total tokens=1665 are comparison
values only; do not fill native data lacking a cache-write validity marker.

Database-wide schema_version=30 gives no per-row client version and cannot verify mixed
historical sessions. The registry remains empty; actual schema fields permit compatibility
reading (latest_fallback). Synthetic auxiliary/backfill/gateway/mixed-model tests do not
become native acceptance. Phase references:
[Hermes container record](../../../../../../../docs/validation/desktop-usage/hermes-container-sample.md).
