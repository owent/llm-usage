# Execution and external operation boundaries

<a id="执行与外部操作边界"></a>

<a id="本地调试与密钥"></a>

## Local debugging and secrets

Disposable scripts, logs and download checks go under ignored repository-root build/task-name/.
Although build/ is ignored at any depth, use the root only. Record owner, paths, processes and cleanup;
existing normal build conventions take priority. The original maintenance task required no external
services or product secrets. Ignore rules for development/secret/ and .env prevent accidental commits;
they do not replace access controls or secret scanning.

Prefer secret managers, system stores or controlled process environments. Credentials never enter
prompts, arguments, logs, debugging, ordinary artifacts or full environment dumps; send them only to
authorized intended services. jq/yq parse data rather than securing transport. Create .env.example
without real values only when configuration needs it. Verify local configuration is ignored before
controlled injection; never request secrets in conversation.

Clean owned task artifacts only. Before recursive deletion/moving, verify resolved absolute paths
and link targets remain inside allowed task directories. Preserve user files, existing changes and
necessary redacted logs/check results. Do not combine destructive commands across shells.

<a id="部署与-ci"></a>

## Deployment and CI

The project has desktop artifacts, SQLite migrations and three-platform build CI. Documentation now
has a separately authorized GitHub Pages publication target under the
[documentation requirements](../../../../docs/design/documentation-site.md).
For actual workflows:

- Establish target environments, artifact identities, migration sequence, health checks,
  acceptance and rollback triggers.
- Use short-lived CI/CD credentials and minimum token permissions. Pin third-party GitHub Actions
  to full commit SHAs. Separate untrusted PR jobs from jobs with publication credentials.
- Report local/mock, real dependencies, staging and production separately. Preparation does not
  establish deployment authorization.
- After timed-out writes, query actual state before retrying.

When a separate test branch is already authorized for commit/push, prepare a reviewed snapshot in an
independent root build/ worktree and preserve the main workspace/other tasks. Do not ask again.
Verify origin, branch and full head_sha against actual workflow triggers. Test-branch pushes do not
trigger this repository's product CI; dispatch explicitly for the branch. After trigger timeouts,
query existing branch/commit runs rather than blindly dispatching again. Record tested source/workflow
revisions, every job and downloaded artifact hashes/reports. Report native tests, packaging and GUI
independently. Subsequent validation-record-only commits must verify no source/workflow changes and validate docs
separately; preceding-revision CI does not verify later commits. Download authentication/signed URLs
never enter arguments/logs.

<a id="mcp"></a>

<a id="mcp-采用门槛"></a>

<a id="mcp-采用条件"></a>

## MCP adoption conditions

This project has no MCP integration. Harness connectors do not establish repository integration.
Do not install servers, change user configuration or start services merely to complete coverage.
Before adoption, record server provenance, flows, tool sets, read/write scope and client/server/SDK/
transport/protocol versions. Verify corresponding official specifications; template current labels
are not verified facts.

Design/acceptance cover:

- Minimal permissions, paths and tools. Validate inputs, targets, symlink boundaries and outbound
  destinations against traversal, SSRF, metadata poisoning and prompt injection in results.
- HTTP follows actual protocol/OAuth audience, issuer, discovery and redirect validation;
  prohibit token passthrough. STDIO uses controlled environments instead of mechanically applying HTTP OAuth.
- Unpredictable business state handles bind to authenticated subjects and reauthorize each call;
  they are not identity credentials. Stateless protocols do not imply stateless business logic
  or establish older SDK behavior.
- HTTP validates Origin, restricts listening and authenticates. STDIO stdout is protocol-only;
  diagnostics use stderr. Prefer protocol cancellation without terminating shared services.
- Bound calls/output/rates and redact audits. Write tools need real permissions.
  Read-only/destructive annotations are metadata, not authorization guarantees.
- Nonsensitive templates may enter Git; tokens, passwords, cookies and session files never enter
  Git/model context. Clean owned process trees on exit and verify processes/listeners are closed.

These are integration checks, not completed protocol implementation/runtime acceptance.
First integration needs version-specific sources and side-effect-free discovery, permission refusal,
error-path and process-lifecycle tests.

<a id="timeouts"></a>

<a id="超时与重试"></a>

## Timeouts and retries

Tool wait windows, subprocess timeouts and the overall task time limit are separate quantities.
Still running means waiting must resume rather than failure. Use finite limits based on actual CI
history, risk and stages; do not give all commands identical timeouts.

After timeouts, retain redacted diagnostics and inspect downloads, deadlocks, resource limits and
remaining processes. Confirm old process state before another attempt. Retry recoverable errors
within bounds, with jittered network backoff/Retry-After. Diagnose parsing/argument/permission errors;
do not retry endlessly or weaken security.

Deployment, creation or sending may succeed despite timeout. Query durable identities/actual state,
then use idempotency keys/deduplication before replay. Report unknown state if ambiguity remains.
Automation never waits for interactive input; users log in through controlled interfaces.
Windows background processes use hidden windows. Record PIDs/logs/exit cleanup and stop owned processes only.
