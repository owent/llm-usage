//! Two parser slots, single writer, job merging and cooperative cancellation.
mod common;
use common::*;
use llm_usage_core::adapters::{codex::CodexAdapter, framework::*, run_policy};
use llm_usage_core::{
    error::CoreError,
    jobs::{RunStart, RunStatus, TriggerKind},
    storage::Storage,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

type Action = Arc<dyn Fn() -> Result<(), CoreError> + Send + Sync>;
struct Adapter {
    id: &'static str,
    inner: CodexAdapter,
    action: Action,
}
impl SourceAdapter for Adapter {
    fn adapter_id(&self) -> &'static str {
        self.id
    }
    fn agent(&self) -> &'static str {
        "codex"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        self.inner
            .discover(ctx)
            .into_iter()
            .map(|mut root| {
                root.basis = RootBasis::EnvOverride("TEST_ROOT".into());
                root
            })
            .collect()
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("{}@{}", self.id, normalize_path(&root.root))
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        self.inner.detect(path)
    }
    fn capability(&self) -> CapabilityTable {
        self.inner.capability()
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, CoreError> {
        (self.action)()?;
        self.inner.scan(target, stored, limits, now)
    }
}
fn source(tag: &str) -> (TempDir, DiscoverContext) {
    let dir = TempDir::new(tag);
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let root = codex_root_with_file(&dir, "rollout-test.jsonl", &jsonl);
    (
        dir,
        DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![root],
        },
    )
}
fn request<'a>(
    adapter: &'a Adapter,
    context: &DiscoverContext,
    n: usize,
) -> ParallelScanRequest<'a> {
    ParallelScanRequest {
        adapter,
        context: context.clone(),
        config: RunConfig {
            timezone: "UTC".into(),
            now_ms: 1_800_000_000_000,
            limits: ScanLimits::default(),
            trigger: TriggerKind::Interval,
            origin_host_id: None,
            run_id_prefix: format!("parallel-{n}"),
        },
        filter: InstanceFilter::default(),
    }
}
fn count(storage: &Mutex<Storage>, table: &str) -> i64 {
    storage
        .lock()
        .unwrap()
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn release(gate: &Arc<(Mutex<bool>, Condvar)>) {
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
}
fn wait(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let (ready, timeout) = gate
        .1
        .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(3), |ready| {
            !*ready
        })
        .unwrap();
    assert!(
        *ready && !timeout.timed_out(),
        "writer lock or worker-slot deadlock"
    );
}

#[test]
fn two_parsers_release_writer_and_preserve_all_commits() {
    let (_db, db) = temp_storage("parallel-two");
    let storage = Arc::new(Mutex::new(db));
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started, received) = mpsc::channel();
    let sources: Vec<_> = (0..4)
        .map(|i| source(&format!("parallel-source-{i}")))
        .collect();
    let adapters: Vec<_> = ["test-a", "test-b", "test-c"]
        .into_iter()
        .map(|id| {
            let storage = storage.clone();
            let gate = gate.clone();
            let active = active.clone();
            let peak = peak.clone();
            let started = started.clone();
            Adapter {
                id,
                inner: CodexAdapter::new(),
                action: Arc::new(move || {
                    let until = Instant::now() + Duration::from_secs(1);
                    loop {
                        if storage.try_lock().is_ok() {
                            break;
                        }
                        assert!(
                            Instant::now() < until,
                            "carrier parsing must release the writer"
                        );
                        std::thread::yield_now();
                    }
                    let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(n, Ordering::SeqCst);
                    started.send(id).unwrap();
                    wait(&gate);
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                }),
            }
        })
        .collect();
    let mut requests: Vec<_> = adapters
        .iter()
        .zip(&sources)
        .enumerate()
        .map(|(i, (a, (_, ctx)))| request(a, ctx, i))
        .collect();
    requests[0]
        .context
        .manual_roots
        .extend(sources[3].1.manual_roots.clone());
    let results = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            run_adapter_scans_parallel(storage.as_ref(), &requests, None, Arc::new(|| true), None)
        });
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 2);
        storage.lock().unwrap().conn().execute("INSERT INTO diagnostics(code,message,created_ms) VALUES('parallel_ui','responsive',1800000000000)", []).unwrap();
        release(&gate);
        worker.join().unwrap()
    });
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(results.len(), 3);
    for result in results {
        let reports = result.unwrap();
        for report in reports {
            assert_eq!(report.finish, RunStatus::Succeeded);
            assert_eq!(report.outcome.as_ref().unwrap().added, 1);
        }
    }
    assert_eq!(count(&storage, "usage_events"), 4);
    assert_eq!(count(&storage, "ingestion_checkpoints"), 4);
    for request in &mut requests {
        request.config.run_id_prefix.push_str("-repeat");
        request.config.now_ms += 1000;
    }
    let repeated =
        run_adapter_scans_parallel(storage.as_ref(), &requests, None, Arc::new(|| true), None);
    assert!(repeated.into_iter().all(|r| r
        .unwrap()
        .into_iter()
        .all(|report| report.files[0].status == "unchanged")));
    assert_eq!(count(&storage, "usage_events"), 4);
}

#[test]
fn same_source_is_merged_before_second_parser_and_single_failure_is_isolated() {
    let (_db, db) = temp_storage("parallel-merge");
    let storage = Mutex::new(db);
    let (_src, ctx) = source("parallel-merge-source");
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();
    let adapter = Adapter {
        id: "test-merged",
        inner: CodexAdapter::new(),
        action: {
            let gate = gate.clone();
            let calls = calls.clone();
            Arc::new(move || {
                calls.fetch_add(1, Ordering::SeqCst);
                tx.send(()).unwrap();
                wait(&gate);
                Ok(())
            })
        },
    };
    let requests = [request(&adapter, &ctx, 0), request(&adapter, &ctx, 1)];
    let results = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            run_adapter_scans_parallel(&storage, &requests, None, Arc::new(|| true), None)
        });
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        // Observe the merge while the owner is still parsing.
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            let merged: i64 = storage
                .lock()
                .unwrap()
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM ingest_runs WHERE merged_triggers != '[]'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            if merged > 0 {
                break;
            }
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        release(&gate);
        worker.join().unwrap()
    });
    let reports: Vec<_> = results.into_iter().map(|r| r.unwrap().remove(0)).collect();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        reports
            .iter()
            .filter(
                |r| matches!(r.start, Some(RunStart::Merged(_))) && r.finish == RunStatus::Running
            )
            .count(),
        1
    );
    assert_eq!(count(&storage, "usage_events"), 1);
    let (_bad, bad_ctx) = source("parallel-error");
    let (_good, good_ctx) = source("parallel-ok");
    let bad = Adapter {
        id: "test-bad",
        inner: CodexAdapter::new(),
        action: Arc::new(|| Err(CoreError::Validation("synthetic parser rejection".into()))),
    };
    let good = Adapter {
        id: "test-good",
        inner: CodexAdapter::new(),
        action: Arc::new(|| Ok(())),
    };
    let results = run_adapter_scans_parallel(
        &storage,
        &[request(&bad, &bad_ctx, 2), request(&good, &good_ctx, 3)],
        None,
        Arc::new(|| true),
        None,
    );
    assert_eq!(results[0].as_ref().unwrap()[0].finish, RunStatus::Failed);
    assert_eq!(results[1].as_ref().unwrap()[0].finish, RunStatus::Succeeded);
    assert_eq!(count(&storage, "usage_events"), 2);
}

#[test]
fn disabling_an_active_source_stops_inside_the_carrier_without_advancing_cursor() {
    let (_db, db) = temp_storage("parallel-disable");
    let storage = Mutex::new(db);
    let (_src, ctx) = source("parallel-disable-source");
    let (tx, rx) = mpsc::channel();
    let adapter = Adapter {
        id: "test-disabled",
        inner: CodexAdapter::new(),
        action: Arc::new(move || {
            tx.send(()).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                run_policy::check()?;
                assert!(Instant::now() < until);
                std::thread::yield_now();
            }
        }),
    };
    let disabled = Arc::new(Mutex::new(std::collections::BTreeSet::new()));
    let requests = [request(&adapter, &ctx, 0)];
    let instance = adapter.instance_id(&adapter.discover(&ctx)[0]);
    let results = std::thread::scope(|scope| {
        let disabled_gate = disabled.clone();
        let worker = scope.spawn(|| {
            run_adapter_scans_parallel(
                &storage,
                &requests,
                None,
                Arc::new(|| true),
                Some(Arc::new(move |id| {
                    !disabled_gate.lock().unwrap().contains(id)
                })),
            )
        });
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let db = storage.lock().unwrap();
        db.conn()
            .execute(
                "UPDATE source_instances SET enabled=0,health='degraded' WHERE instance_id=?1",
                [&instance],
            )
            .unwrap();
        disabled.lock().unwrap().insert(instance.clone());
        drop(db);
        worker.join().unwrap()
    });
    assert_eq!(
        results[0].as_ref().unwrap()[0].finish,
        RunStatus::Interrupted
    );
    assert_eq!(count(&storage, "usage_events"), 0);
    assert_eq!(count(&storage, "ingestion_checkpoints"), 0);
    assert_eq!(
        storage
            .lock()
            .unwrap()
            .conn()
            .query_row(
                "SELECT health FROM source_instances WHERE instance_id=?1",
                [&instance],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "degraded"
    );
    assert!(
        run_adapter_scans_parallel(&storage, &requests, None, Arc::new(|| true), None)[0]
            .as_ref()
            .unwrap()
            .is_empty()
    );
}
