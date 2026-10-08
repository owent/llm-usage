# synthetic-not-pi._expectations.md (synthetic)

<a id="synthetic-not-pi_expectationsmdsynthetic"></a>

**All files are synthetic, not native session extracts.** V17 rejects a non-Pi format
whose first record is not a session header.

<a id="场景与期望"></a>

## Scenario and expectations

- First line {"type":"event_msg",...} resembles Codex rollout. Even a valid session
  header on line 2 does not qualify; detection checks the first line only.
- detect=UnknownFormat("first record type is not session header").
- Run: status=unknown_format, no events, one unknown_format diagnostic;
  source_files.status=unsupported.
