---
title: Add or update an adapter
description: Establish version references and preserve unknown values, identity and recovery.
sidebar:
  order: 3
---

<a id="specify-local-formats-and-fields"></a>

## Specify record formats and fields

Read the native writer, configuration and fixed-version upstream source before coding.
Record the product/version, exact local layout, environment overrides, schema, timestamps,
record identity and field meanings. Use official installation discovery tools or registry
data where appropriate rather than guessing edition/year paths.

A nonempty native sample verifies only its own product/version/client interface. Shared kernels,
database-wide versions, installed binaries and empty sessions do not verify historical
rows or another product. Preserve unsupported transport/archive/provider boundaries.

## Implement independently

Put each agent under `adapters/<agent_id>/`, with version implementations inside that
directory. Extend the existing registry and discovery routes. Give application-managed
roots a route based on file/database format, and preserve a single owner for each physical file.
Do not hide a manually selected file's real format error by suffix heuristics.

Map every field with `reported`, `derived`, `estimated` or `unknown` basis. Determine whether
input includes cache and reasoning is an output subset. Before treating an initialized zero as
reported usage, verify that the client received and saved that field. Keep calls, messages, observations,
cumulative snapshots and quotas separate. Never infer a billing channel from a model name.

## Recovery and regression results

Check duplicate reads, replacement by revision/lifecycle order, conflicts at equal rank, whole-batch
rollback, malformed records mixed with valid ones, bounded reads/cancellation and old
consumed processing positions. Parser corrections may update old observations only when
the complete old summary matches the specifically authorized rule change. Preserve
first observation time, conflict history, diagnostics and sealed partitions.

Verify discovery through the complete registry, including parent-directory promotion,
custom roots and old database ownership restoration. A parser-only unit test cannot
establish these behaviors.

Update the [matrix](/reference/design/adapters/), [research index](/reference/design/research/),
affected [data rules](/reference/design/data-contract/) and targeted acceptance record
in both languages. Label synthetic test data and real samples distinctly; redact to the
statistical allowlist. Read [testing](/development/testing/) for validation scope.
