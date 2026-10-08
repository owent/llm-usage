---
title: Price references and usage alerts
description: Distinguish current API references, historical estimates and real bills.
sidebar:
  order: 6
---

## Three different amounts

| Amount | Interpretation |
| --- | --- |
| Source-reported cost | The local client reports an amount; it may itself be a client estimate. Keep its currency and basis. |
| Historical estimate | Usage priced with a stored snapshot at observation time. Later price updates do not rewrite it. |
| Current API reference | A comparison using the current verified pay-as-you-go price, not your subscription bill or proof of your billing channel. |

Cost estimation is disabled by default. Unknown models, ambiguous channels, missing
fields and incomplete tiers remain visible gaps. Currencies are not merged. Cache read,
cache write, input, output and price thresholds use the source's verified semantics.

If an exact channel price is unavailable, the product can show a clearly identified
official API reference for the same verified model. Model-series similarity is not enough.
The explicitly authorized Kimi K2.8 Preview to K2.7 Code reference is labeled as a substitute;
it does not change model identity or historical amounts.

## Optional online prices

Price refresh is off by default. The built-in source is `models.dev/api.json`; the request
sends no local usage data. Raw responses have a long-lived cache (three days by default,
configurable from 1–365), and download/validation failure falls back to the last successful
cache. Only eligible official pay-as-you-go entries are imported; subscription placeholders
do not become prices.

## Usage and cost alerts

Usage and cost alerts are off by default. Choose a local day or month and a known-token threshold,
or a single-currency historical-estimate threshold. Monetary reminders require estimation
to be enabled. The reminder appears within the app and does not stop an agent or collection.

A known lower bound reaching the threshold can trigger a reminder. Not receiving a reminder
does not prove complete usage stayed below the configured threshold. The UI explains partial usage or price
coverage. A saved user/timezone/period/metric/currency/threshold identity prevents repeated
notifications after rescanning or restarting.

Exact arithmetic, archive scope, exchange references, exceptions and quality rules are in
the [pricing rules](/reference/design/pricing/), [dashboard repair specification](/reference/design/dashboard-repair/)
and [usage and cost alert rules](/reference/design/budget-reminders/).
