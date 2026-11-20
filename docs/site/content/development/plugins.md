---
title: Create a model name plugin
description: Write a model alias catalogue, build a local plugin package, and test it in Settings.
sidebar:
  order: 3.5
---

Use a desktop build with **Settings → Model mappings → Model name plugins**. The current package format
supports one offline model name catalogue. An alias supplies a display label for existing local usage.
Agent collectors, price sources, online feeds and exporters are tracked in the [plugin design](/reference/design/plugins/).

The commands below use PowerShell 7, a `tar` that supports USTAR, and `zstd`. Use a directory for your plugin sources.

## Write the catalogue

Save this example as `aliases.json` in UTF-8 without a byte order mark. `example-preview` is a
synthetic model key. For your first working rule, use a key and Agent ID already present in your records.

```json
{
  "schema_version": 1,
  "rule_version": "example-2026-11-16",
  "published_at": "2026-11-16T00:00:00Z",
  "aliases": [
    {
      "agent": "codex",
      "key": "example-preview",
      "display_name": "Example Preview",
      "basis": "catalogue",
      "effective_from": "2026-11-16T00:00:00Z"
    }
  ]
}
```

| Field | How to choose it |
| --- | --- |
| `schema_version` | Use the integer `1`. |
| `rule_version` | Name this revision of your catalogue. Update the label when you revise the rules so you can identify the catalogue in Settings. |
| `published_at` | Use a UTC timestamp ending in `Z`. The host allows at most five minutes ahead of its clock. |
| `agent`, `key` | Use the recorded Agent ID and raw model key. Both compare case-insensitively, and the key follows the application's model spelling normalization. Preserve release suffixes and namespaces. |
| `display_name` | Set the label you want to see in the dashboard and record details. |
| `basis` | Use `catalogue` for every plugin rule. |
| `effective_from`, `effective_until` | Use UTC timestamps ending in `Z`. The start is inclusive and the optional end is exclusive. Choose a start at or before the usage you want to label. Omit the end for an open interval. |
| `provider_id` | Optional. Use the recorded provider ID to narrow a rule to that provider. Omit the field to match across providers for the specified Agent and model key. |
| `source_urls` | Optional. Add up to eight public HTTPS references without query parameters. The URLs document the rule's source. |

Use a detail exchange export to inspect `agent`, `model_raw` and `provider_id` when the dashboard already
shows a formatted label. Choose a record with a nonempty raw model key. The catalogue must contain
1–2,000 rules and is limited to 1 MiB. [Export and import](/guide/data/). [Model mapping reference](/reference/design/model-aliases/).

## Generate the manifest and package

Run this block from the directory containing `aliases.json`. Replace `example.model-names`, `Example`
and `MIT` with your own plugin ID, publisher and actual licence. Start with a SemVer version such as `1.0.0`.
The plugin ID contains at least two dot-separated segments of lowercase ASCII letters, digits and single hyphens.
Each segment starts and ends with a letter or digit. `builtin.` and `llm-usage.` are reserved prefixes.

```powershell
$version = '1.0.0'
$catalogHash = (Get-FileHash -LiteralPath aliases.json -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = [ordered]@{
    format = 'llm-usage-plugin/1'
    api_major = 1
    id = 'example.model-names'
    version = $version
    name = 'Example model names'
    publisher = 'Example'
    license = 'MIT'
    contribution = [ordered]@{
        id = 'names'
        kind = 'model_aliases'
        entry = 'aliases.json'
        sha256 = $catalogHash
    }
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath manifest.json -Encoding utf8NoBOM

$archive = "example-model-names-$version.tar"
tar --format ustar -cf $archive manifest.json aliases.json
if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
zstd -1 -f $archive -o "${archive}.zst"
if ($LASTEXITCODE -ne 0) { throw 'zstd failed' }
```

The output is `example-model-names-1.0.0.tar.zst`. The archive contains exactly two regular files at
the root, `manifest.json` and `aliases.json`. The limits are 2 MiB for the compressed archive and
16 KiB for the manifest. Archive only those two files. The command hashes the final catalogue bytes,
so rerun it after any edit, including a change in line endings.

## Install and check the result

1. In **Settings → Model mappings → Model name plugins**, click **Choose plugin package** and select
   the generated `.tar.zst`.
2. Check the ID, version and rule count in the preview. The **Unverified publisher** label is expected
   for a local unsigned package. The digest in the preview identifies the manifest bytes, which include
   the catalogue digest.
3. Click **Install disabled**, then **Enable this version**.
4. In **Details**, select usage for that Agent and model inside the effective interval. Check the displayed
   label. For a dashboard summary, select a period fully covered by one name. The raw model key continues
   to identify the usage, and costs use the recorded pricing identity.

## Update, roll back or remove

Keep the same plugin ID when publishing another version. Set a new `$version` in the packaging block
whenever either input file changes. Install the new package and enable its version. The app activates one
version per plugin ID. To roll back, enable the previous installed version. **Disable** withdraws the
labels. **Remove** removes the version from the installed list and retains its inactive catalogue history.
Reinstalling the same manifest and catalogue is idempotent.

## Diagnose a failed package or a missing label

| Symptom or code | What to check |
| --- | --- |
| `unsupported_manifest` | Confirm that `format` is `llm-usage-plugin/1`, `api_major` is the number `1`, and `contribution.kind` and `contribution.entry` are `model_aliases` and `aliases.json`. |
| `invalid_manifest` or `model_alias_json` | Check UTF-8 without a byte order mark, strict JSON, the required fields and the validity of the plugin ID and version. Use the exact field names in the examples. |
| `asset_digest_mismatch` | Hash the final `aliases.json` bytes again and rebuild the archive using the packaging block. |
| `invalid_archive_member`, `duplicate_archive_member`, `missing_manifest` or `missing_aliases` | Rebuild with the exact lowercase filenames at the root, without a containing folder, links or duplicate entries. |
| `invalid_plugin_alias_basis` or `model_alias_rule` | Set `basis` to `catalogue`. Check interval order and any optional provider and source URL fields. |
| `model_alias_future_publication` | Check the computer clock and the UTC value of `published_at`. |
| `package_size_limit` | Check the compressed size and the size of each source file. Keep only the two declared members. |
| `package_version_conflict` | Keep the original package for that ID and version. Give the modified package a new version and rebuild it. |
| Enabled, but the label is absent | Check the recorded Agent, model and provider, then narrow the date range. Applicable built-in and already imported mappings take priority. Coverage gaps or conflicting plugin labels can leave the raw model key visible. A database-only copy may also require enabling the package on the current machine. |

See [package and storage limits](/reference/design/plugins/#local-model-name-packages) for the host's behavior and limits.
