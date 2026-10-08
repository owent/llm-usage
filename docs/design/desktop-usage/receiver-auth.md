# Local HTTP receiver authentication

<a id="本机-http-接收鉴权"></a>

Scope: current-user local Claude Code/Codex instances where the user explicitly applies
configuration, plus CodeBuddy CLI 2.98.0 with a manifest verified at the same installation
path. The receiver remains off by default and binds only 127.0.0.1. File export needs no
receiver token. Official CodeBuddy 2.98.0 release notes confirm OTLP authentication headers;
its manifest restricts automatic setup to that version. Older/newer versions, missing
manifests and nonstandard installations need manual checks.

Source identity, binding and revocation rules are shared across platforms. Linux uses
the current user's Secret Service default persistent collection with a DH-encrypted
session. No fallback to a session collection, plaintext file or in-memory credentials;
even the default alias cannot point to session. Inspection creates/unlocks no collection.
Absent service/default collection, locked collection or duplicate matches reject setup.
Each storage operation has a three-second limit; background reads show no unlock prompt.
macOS uses current-user Keychain generic passwords, explicitly disables iCloud sync and
forbids authentication UI on every query. Locked/denied/unavailable storage rejects setup.
Both use system randomness for tokens. Errors return stable codes, without formatted
library errors or credentials. Record implementation, native storage roundtrips and
actual desktop exporter acceptance separately.

Each applied configuration generates a separate 128-bit source ID and 256-bit random
token. Windows uses BCrypt system randomness and a current-user Generic Credential with
local-machine persistence. Native parallel tests reproduced successful writes followed
by an immediately missing read and matching content after 10 ms. Credential creation,
failure cleanup and pre-revocation reads wait only on missing content: at most five
additional reads, 10 ms apart. Mismatches/read errors reject immediately, without rewriting.
Authentication requests still read once and reject missing credentials. This observation
does not establish API consistency for all Windows environments. After deletion, verify
absence before reporting success. If Windows still returns exactly the owned content,
wait at most the same 50 ms without deleting again. External replacement, read errors
or timeout report failure and leave other sources intact.

Credentials also store application data directory, client and configuration path.
Requests read the credential by source ID, compare tokens with constant work and check
the application directory and permitted HTTP path. Restart does not weaken authentication.
Missing credentials, unavailable storage and missing/duplicate/invalid headers reject
before body reads/decompression. Reject browser Origin and requests declaring forwarding.
This cannot identify a token-holding process or prevent deliberate same-user forwarding;
only user-confirmed local Agents are supported.

Claude uses logs-specific headers; Codex uses exporter headers. Both permit only
`/v1/logs`. CodeBuddy 2.98.0 uses generic headers confirmed in its release notes,
encoding the Bearer space as %20 per the official example, and permits only
`/v1/traces/supplemental` in isolation. Later rolling documentation's traces-specific
headers do not establish old-version support; an existing such key needs manual review.
Preserve remote/other output destinations without injecting this application's token.
User configuration must contain an exporter-readable token; the system credential store
holds the application's authentication copy. Tokens therefore are not confined to the
credential store. Configuration, conditional rollback backups and revocation information
stay local. Previews show placeholders only; inspection generates/writes no credentials.
IPC, usage databases, exports and diagnostics return no tokens.

Verify preview version, bind the actual receiver, create credentials, then write user
configuration. Failure removes this attempt's credentials and restores its enable state.
Undo revokes the credential first, then restores only keys still belonging to this setup.
Report revocation failure rather than claiming successful undo. If a configured token
becomes invalid, offer reconfirmation only for destinations still pointing to this
application; never change user files automatically. Undo handles remain process-local;
persistent undo after restart and complete installation lifecycle need separate verification.

References checked: exporter/Windows on 2026-10-04, cross-platform storage on 2026-10-05.

- [Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage): logs headers, user env and authentication.
- [Codex sample configuration](https://developers.openai.com/codex/config-sample/): otlp-http headers table.
- [OTLP exporter specification](https://opentelemetry.io/docs/specs/otel/protocol/exporter/): per-signal headers and key/value format.
- [CodeBuddy 2.98.0](https://www.codebuddy.ai/docs/cli/release-notes/v2.98.0),
  [environment variables](https://www.codebuddy.ai/docs/cli/env-vars),
  [installation](https://www.codebuddy.ai/docs/cli/installation): indexed official page bodies
  verified version/headers/package name. Direct requests failed, so no successful live
  download is claimed. Unverified versions do not inherit automatic setup.
- [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom): system randomness.
- [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew),
  [CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw),
  [CredDeleteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creddeletew): user scope, persistence and precise cleanup.

Synthetic HTTP, failure recovery and native Windows credential-store tests are distinct
from actual product-export acceptance. Linux native storage/HTTP and macOS cross-type
checks are in the [cross-platform record](../../validation/desktop-usage/platform-auth-continuation.md).
Native cross-process macOS Keychain roundtrips and HTTP revocation are in
[this batch's CI](../../validation/desktop-usage/ci-plan-validation.md). Actual desktop/exporter
acceptance remains separate. Windows parallel storage reproduced temporarily missing
post-write reads, leading to limited mutation readbacks/deletion confirmation. First
failures, cross-process revocation anomalies and final reruns are in
[source-rule continuation](../../validation/desktop-usage/source-policy-upgrades.md).
