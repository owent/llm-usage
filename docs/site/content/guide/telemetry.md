---
title: Telemetry exports
description: Check and enable verified exports without duplicating native records.
sidebar:
  order: 5
---

Native history does not always contain every token field. Supported clients can provide
additional local OTel file exports or explicitly authenticated local HTTP telemetry.
Each client interface and version requires independent source and sample verification.

## Inspect before applying

The overview offers a compact configuration summary; **View details** opens the local
telemetry panel in Settings. Checks are read-only, run in the background and distinguish
missing configuration, configured-but-waiting, verified samples and conflicts. A leftover
config directory alone does not establish an installed client.

Review a single client's configuration preview, or use the batch action for installed,
supported items missing configuration. Applying merges user-level fields and preserves
existing output targets and unrelated settings/comments. Conflicting and manual-only
items are reported separately. Reload an IDE when required; a generated CLI launcher
must be used deliberately and is not automatically executed.

## File output and overlap

File output can continue while LLM Usage is closed and be collected later. It does not
recover requests that occurred before export was enabled, and the client's own retention
can remove historical files.

Verified VS Code Copilot file spans replace eligible native contributions by host, user,
session and local day. Original records remain stored; approximate timestamps or equal
tokens do not establish call identity. Sealed partitions keep their chosen contribution.
Other exports are isolated until their shape and overlap are independently verified.

## Local HTTP receiver

The receiver is off by default and binds only to `127.0.0.1`. It accepts verified OTLP
JSON/protobuf traces and logs over HTTP; it does not support gRPC or metrics. The app must
be running to receive data.

Each configured source requires its own credential in the OS credential store. Previews
and IPC never return the secret. If persistent secure storage is unavailable, setup is
refused rather than opening an unauthenticated receiver. Revocation verifies removal of
the owned credential and rejects future requests.

Configuration capability does not verify actual exporter output. Read the
[telemetry requirements](/reference/design/copilot-otel/) and
[receiver authentication requirements](/reference/design/receiver-auth/) for exact client
versions, output names, manual routes, rollback and credential boundaries.
