---
title: Privacy and data boundaries
description: What is read, what is stored and when a network request is made.
sidebar:
  order: 8
---

LLM Usage collects the statistical allowlist from verified local agent files/databases. It stores
normalized usage, provenance, revisions, diagnostic facts and collection positions in a
local SQLite database. The dashboard and normalized exports do not contain conversation
bodies, responses, tool output or account credentials.

Reading a local record of a cloud model request remains local usage accounting. It does
not mean the model ran offline. Remote billing APIs, cross-device account reports, SSH
gateways and cloud-synchronized sessions are outside the local-source boundary.

## Network behavior

- Statistics do not require an extra model request, account login or cloud database.
- Optional price refresh requests the built-in public price catalog and sends no local
  usage. It is disabled by default.
- Optional authenticated OTLP/HTTP reception listens only on loopback and requires the
  running app. It is disabled by default.
- Explicit IDE telemetry configuration can change how that client exports locally;
  inspect the preview and preserve its existing targets.
- The documentation site uses local search and bundled assets, with no analytics or
  remote font requests. Following an external source link opens that external site.

## Credentials and sharing

Receiver credentials live in the OS store and are never returned by configuration previews
or IPC. Setup fails closed when secure persistent storage is unavailable. The product
does not ask for a model-provider secret to read existing usage files.

Usage exports contain stable source/host identity. Screenshots of your own Sources or
Settings pages can expose paths and aliases. Review these before sharing. The documentation's
screenshots use isolated synthetic sources and a demo user; their provenance is recorded
alongside the images.

The authoritative [data rules](/reference/design/data-contract/) and
[receiver requirements](/reference/design/receiver-auth/) describe admissible fields and exact
failure behavior. A diagnostic can explain a missing record without revealing its body.
