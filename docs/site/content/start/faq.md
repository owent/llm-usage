---
title: Frequently asked questions
description: Usage collection, billing, unknown values and supported clients.
sidebar:
  order: 3
---

## Does the app call a model to calculate statistics?

No. It reads existing local records and optional local telemetry. Statistics require no
additional language-model request, model-provider login or cloud database.

## Are these totals my bill?

No. Native usage, source-reported amounts, observation-time estimates and current API
references have different meanings. Subscription quotas are separate. A model name does
not establish your billing channel, and incomplete fields remain visible.

## Why are some values blank?

The source did not provide a verified value. Blank/unknown is not zero. Some clients only
record a lower bound or an aggregate observation; the app does not fabricate per-call
counts, cached buckets or model attribution from those records.

## Does finding an installed client mean it is supported?

Installation, configuration, parser implementation and real-token acceptance are distinct.
Check the adapter matrix for the exact product/version/format. A document-level parser
can exist without a native real sample, and an empty session is not a format acceptance test.

## Can I collect WSL, container or remote account data?

Local WSL/container sources require explicit roots and instance attribution. The app does
not automatically launch or scan those environments. Remote billing/account reports,
SSH gateways and synchronized remote sessions are outside the local-source boundary.

## Does automatic refresh keep running after I exit?

In-process collection ends with the process. Windows background collection requires explicit
enablement and verified task definitions. A window closed to the tray is a separate state.
Automatic interval zero blocks automatic triggers; manual refresh remains available.

## Which language is the default?

Repository documentation/comments and the documentation root default to English. The site
selects a supported browser language on first entry and saves manual choices. The desktop
app has ten language catalogs and its existing default is Simplified Chinese; its settings
are independent of the documentation.
