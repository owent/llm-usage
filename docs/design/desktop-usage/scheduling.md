# Local scheduled collection and background lifecycle

<a id="本地定时提取与后台生命周期"></a>

Current implementation provides global intervals, per-source interval/daily/weekly rules,
startup reconciliation, merged manual requests, one database writer and optional native
Windows tasks. Optional Windows tray, energy-saving pause and cooperative scan limits
are implemented; acceptance status in [Plan](../../../Plan.md). Windows file watching and
monotonic per-source intervals are implemented. Two source-instance parsing slots use
scoped cancellation for file/database reads and postprocessing.

<a id="设置与执行选择"></a>

## Settings and execution

| Setting | Default and current behavior |
| --- | --- |
| Global automatic collection | One hour; 0 disables every automatic trigger, including startup/per-source/system triggers; manual collection remains available |
| Per-source rule | Overrides global; interval 15 seconds–24 hours, daily HH:MM or weekly ISO weekday + HH:MM; source must be enabled |
| Calendar-rule timezone | Initially statistical timezone, then saved independently; changing statistics does not change schedule. Source page can change it and preview next three runs |
| Startup/resume | With automation enabled, reconcile all enabled sources at startup; multiple missed global intervals become one run |
| Start at login | Current Windows user, off by default; configured separately from background tasks |
| Collect after exit | Windows system task off by default; checks due sources without a window while current user logged in; no waking computer |
| Tray | Windows off by default; enabled close hides window; tray/show restores; exit menu always exits. Login startup/system collection independent |
| Energy-saving pause | Windows on by default; read-only GetSystemPowerStatus; pauses startup/interval/per-source/system automation. Manual remains; reconcile once after resume |
| File watching | Optional Windows, off by default; registered/enabled local roots only, two-second debounce; excludes daily/weekly rules; polling remains fallback |

Windows login startup uses native registry APIs for the app's HKCU Run value. Status verifies
current executable path; old paths/unexpected value forms do not count as active. No reg.exe;
delete-permission failures do not count as disabled.

All sources share collection entry points. Tasks read existing local records without shell,
agent CLI, prompts or model calls. Manual refresh includes every enabled source, including
independent schedules. Automatic global scans exclude independent schedules not yet due;
each due source gets one additional scan. Disabling a source retains historical contributions.
After disabling app automation, residual system triggers cannot read. Explicit --scan-once
is manual collection.

Saved schedules include rule/timezone/enabled state/configuration version/next run. Stopped
sources are not read automatically; resumed missed times merge into one reconciliation.
DST missing calendar times move to the first valid time after the transition; repeated times
use the first occurrence. Completed rules advance to the next calendar time, without replay
after clock rollback. Global/per-source interval waits use monotonic clocks while running.
Initial per-source load restores persisted UTC due times; later clock jumps do not trigger
intervals early. Completion/rule revision restarts timing; remaining monotonic time corrects
the displayed next UTC time. Daily/weekly rules still use their independent timezone's calendar.
GUI startup/manual/global scans share the persisted global deadline; per-source scans advance
only their source, avoiding repeated global resets that postpone inherited-global sources.
Legacy rules lacking timezone bind the current effective statistical timezone once at initialization.

<a id="合并事务与恢复"></a>

## Merging, transactions and recovery

GUI/headless share two source-instance slots, permitting different roots of the same agent
in parallel. Empty slots take the next root; existing job state merges same-instance requests.
Release app database lock during reads/parsing. One Storage mutex serializes file ownership/
cursor loading/result acceptance/batch/job-state updates; no second writer connection.
Archive replacement uses that lock. Parallel regressions cover same-file ownership,
disable/pause, independent source failures, event/cursor rollback and duplicate-free rescans.

Each worker has scoped cancellation with the earlier source/round deadline. Bounded reads,
line/record loops, SQLite queries and backup pages check it. Accept only complete valid
batches. Interruption is not a parse error and cannot advance unprocessed cursors; scoped
control/source SQLite handles released with the scope. Successful detection/generation and
events/cursors commit together; same-size replacement after failure still requires rereading.
JSONL default per-file window 32 MiB; explicitly larger line limits enlarge it accordingly.
Complete lines can commit; remainder marked interrupted, source schedule stays due. Reading
uses at most half the remaining per-file time to leave parsing/commit time. Pause/expiry
rolls back unaccepted batches. Newline searches examine newly read bytes only.

Codex prioritizes unvisited files across runs, rotating others by last confirmed visit time
in the database; ties use reverse date-path order. Large files retain cursors after exhausting
the window and yield to other unvisited files next run, preventing historical backfill from
postponing fresh usage. Rotation does not mark interrupted collection complete. Other
adapters keep their discovery order; enable rotation only after verifying files have no
read-order dependency.

Controlled JSON checks every 32 KiB, IO reads at most 64 KiB. Switch checks cached for at
most 20 ms; deadlines checked each time. SQLite checks every 1,000 VM instructions and
backup page group; callbacks execute no SQL. Archive/retention/cost callbacks removed
before unlocking; unaccepted maintenance transactions roll back. Interrupted rounds do
not start retention/cost/optional price refresh. Unvisited/interrupted/merged requests do
not advance per-source due times. Blocking OS calls can only be checked after return;
no hard real-time termination guarantee.

GUI/headless use an OS file lock for the same database's sole owner; OS releases on abnormal
exit. Losing manual/system requests persist and merge separately; opening Storage must not
recover another owner's running jobs. GUI handles manual requests independently of automation;
system requests recheck saved intent. Repeated manual refresh during a run retains at most
one follow-up request. Due calendar rules stay in the database, advancing after execution.

Discovery groups adapters by registry; two slots collect instances with one writer. Events,
cursors/parse state/aggregates/job committed progress share transactions. Failed sources
retain state while other adapters continue. Restart recovers interrupted jobs with stable
rescans, without PID files indefinitely holding ownership. Task registration and collection
success are separate states.

Lifecycle rules and implementation limits:

- Windows directory notifications add no scanning thread. Global watch switch off by
  default; depends on automation and energy-saving checks. Only registered enabled local
  directories/manual-file parents; no daily/weekly watches. Manual-root isolation also
  restricts watch roots. At most 32 notification handles; failures/overflow use polling.
  Notifications alone do not establish usage/read success. Merge one source trigger after
  two quiet seconds; pause releases handles, resume reconciles startup.
- Per-source intervals use in-process monotonic deadlines; short-lived system tasks restore
  UTC on restart, without claiming monotonic clocks persist across processes.
- At most one job/one merged trigger per source; at most two parsing jobs across sources.
- One-file scans retry SQLite busy/locked, transient IO and Windows sharing violations up
  to three times, waits 5/15/30 seconds. Insufficient remaining source time returns error
  immediately. No same-round retry for parsing/permission denial/missing files; discovery/
  detection Pending waits for another round. Retry waits check pause/deadline every 100 ms
  and before rereading. Interrupted scans retain cursor without parse-failure status.
- Settings save on a background worker. Valid pause requests block automatic reads before
  waiting for the write lock. Persisted settings replace only after successful save;
  failure releases the request and keeps old settings. Concurrent pause requests counted separately.
- Source file scans share a default 30-second monotonic limit, including detection/retry
  waits. JSONL receives remaining time; files and inner reads stop on expiry. Only committed
  batches count as accepted; uncommitted events/cursors/detection information roll back
  together. Automatic rounds share five minutes/pause, including reads/archive/retention/
  costs. Unvisited sources are not successful and keep due times. System task has another
  five-minute termination limit. These are cooperative checks, without forcing every OS
  read to return at an exact deadline.
- Tray/energy-saving behavior is Windows-only initially. Other platforms build but native
  behavior is unverified.

Collection sees only saved source records; it cannot restore upstream deletions/unsaved
exports. Cross-day cumulative observations retain uncertain intervals, without assigning
all increments to wake day. Freshness targets require saved sources and normal operation;
see [resource targets](architecture.md#budgets).

<a id="windows-原生系统任务"></a>

## Native Windows system tasks

Register through Task Scheduler COM API, with separate task names/ownership markers for
each app database path. Executable path and arguments saved separately: absolute executable,
--headless --data-dir followed by an absolute directory. No shell concatenation; Chinese/
space-containing paths independently tested. Never replace/delete tasks with mismatched ownership.

Tasks use TASK_LOGON_INTERACTIVE_TOKEN/TASK_RUNLEVEL_LUA, without passwords, SYSTEM or
administrator rights. Check app rules every minute, no waking machine, at most one instance,
OS execution limit five minutes. No execution promise during logout/shutdown/sleep;
merge missed scans after resume. Subminute per-source intervals are limited in system-task-only
mode; UI displays that precision.

Persist enable/disable intent first, call OS, then inspect actual effective state; failures
retain intent/reason. Settings separately show installed/actually enabled, desired state and
retry action. Read-only status does not register tasks. Disabling persists first; residual
--headless exits immediately even if deletion fails. Invalid/changed executable paths after
upgrade show repair-needed; user applies registration again. Task existence alone is not success.

While GUI owns the lock, headless submits one background request and exits; GUI selects due
sources by saved rules. Independent headless also checks intent/global pause/source due times,
then exits without WebView/agent/temporary OTLP receiver. Persisted global background deadline
prevents frequent system triggers from repeating full scans; independent not-due rules keep
their deadlines. Legacy generic-name tasks are not assumed to be newly owned tasks.

--data-dir isolates only application data, leaving source scope unchanged. Native acceptance
must also isolate child source/configuration environments; see [implementation scope](implementation-readiness.md).
OS-launched tasks cannot be assumed to inherit test-process environment. manual_roots_only
off by default; enabled discovery reads only saved manual directories, excluding environment/
default roots, local account quotas and managed telemetry exports. Old statistics retained.
Headless native tests enable it, save nonempty synthetic roots, and independently inspect
events read-only before restarting GUI, preventing startup reconciliation from hiding failed
system collection. V24 checks disable/upgrade/uninstall, registration/deletion failures and
ordinary-user contention. Native registration round trips and actual OS-triggered launches
are reported separately. Standalone Windows uninstall precisely cleans this installation's
owned tasks/startup entries and disables background intent under the existing DB writer lock.
Temporary upgrade uninstall retains them; failure blocks executable removal. See
[installation lifecycle](installation-lifecycle.md).

<a id="其他平台与本机环境"></a>

## Other platforms and local environments

macOS/Linux retain shared scheduler/headless/read/database-lock code and CI. Initial release
has no launchd/systemd/cron registration UI. User explicitly adds WSL/container sources;
discovery does not launch environments. Do not copy live WSL SQLite's three files across
environments. If consistent read-only access is unavailable, use a verified consistent local
export or retain the limitation. Linux app DB belongs on a Linux filesystem, without concurrent
sharing with Windows.

<a id="依据与验收"></a>

## References and acceptance

[Task Scheduler API](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page),
[Windows power status](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getsystempowerstatus),
[Tauri tray](https://v2.tauri.app/learn/system-tray/),
[Windows directory notifications](https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-directory-change-notifications),
[task identity/permissions](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks),
[StartWhenAvailable](https://learn.microsoft.com/en-us/windows/win32/taskschd/tasksettings-startwhenavailable),
[repetition interval](https://learn.microsoft.com/en-us/windows/win32/taskschd/repetitionpattern-interval).
StartWhenAvailable may delay startup and does not guarantee immediate collection after waking.
OS locks use [File.try_lock](https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.try_lock),
minimum Rust 1.89; actual CI toolchains come from repository configuration. Two slots use
[Rust scoped threads](https://doc.rust-lang.org/std/thread/fn.scope.html). Controlled reads
follow [Read errors](https://doc.rust-lang.org/std/io/trait.Read.html#method.read_to_end), returning
typed Other for pause so read_to_end cannot automatically retry Interrupted. SQLite control:
[progress handler](https://www.sqlite.org/c3ref/progress_handler.html). Locked access follows
[rusqlite 0.40.2 Connection](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html)
Send/non-Sync constraints. Official text/locked versions/current compilation and regressions
checked 2026-10-04; rolling Rust documentation is not the installed toolchain. V23/V24/V25
cover pause/due/DST/restart/contention/source scope/headless runs. GUI/headless resources
measured separately; actual results recorded only in validation records and Plan.md.
