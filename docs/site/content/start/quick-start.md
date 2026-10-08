---
title: Quick start
description: Install the desktop client and check your first collection.
sidebar:
  order: 1
---

LLM Usage reads usage already recorded by agents on this computer. You do not need to
sign into your model provider or make an additional model request to view statistics.

## Install and open

1. Read the [installation guide](/start/installation/) and obtain a package appropriate
   for your platform from the repository's verified build artifacts, or build locally.
2. Open LLM Usage. In **Settings**, choose your language, theme and statistical timezone.
3. Open **Sources** to inspect discovered agents, their paths, versions and collection status.
4. Use **Refresh** to collect enabled sources. Inspect both the results and any coverage notes.
5. Open **Overview** for today, **Trends** for a longer period, and **Details** to inspect
   the normalized usage records behind the charts.

The desktop application supports ten languages. Its existing default is Simplified
Chinese; the documentation defaults to English and chooses Chinese for a supported
Chinese browser language. Documentation language does not change application settings.

## Confirm your first result

A source is useful only when its native file/database contains a nonempty usage record.
Installing an agent, finding its directory or opening an empty session does not prove
that tokens can be collected. A request can also be recorded without known token fields.

If no data appears, check the selected user, time range, source enablement, data path
and source status. Follow [troubleshooting](/guide/troubleshooting/) before changing or
deleting the application database. Unknown fields are displayed as gaps rather than zero.

[![English overview with isolated example usage](/screenshots/en/overview-light.png)](/screenshots/en/overview-light.png)

*Actual application UI in English. The example records are synthetic and isolated from
personal agent data; they illustrate the interface and do not verify a client version.*

## Optional settings

Collection uses a global interval of one hour by default. Set the interval to zero to
disable automatic collection; manual refresh still reads enabled sources. Windows login
startup and background tasks, cost estimates, price downloads, the local HTTP receiver
and usage and cost alerts require explicit enablement. See their dedicated guides before use.
