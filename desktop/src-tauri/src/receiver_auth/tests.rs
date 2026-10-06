use super::*;
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub values: Mutex<BTreeMap<String, Vec<u8>>>,
    pub writes: AtomicUsize,
    random_counter: AtomicUsize,
}
impl Store for MemoryStore {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.values.lock().unwrap().get(name).cloned())
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.values
            .lock()
            .unwrap()
            .insert(name.into(), bytes.to_vec());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        self.values.lock().unwrap().remove(name);
        Ok(())
    }
    fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        let n = self.random_counter.fetch_add(1, Ordering::SeqCst) as u8;
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(n);
        }
        Ok(())
    }
}
pub(crate) fn root(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/plan-continuation/auth-tests")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn credentials_bind_app_source_config_and_route_and_revoke_without_restart() {
    let store = MemoryStore::default();
    let app = root("binding");
    let other = root("other-app");
    let config = app.join("claude/settings.json");
    let a = issue(&store, &app, &config, Family::Claude).unwrap();
    let b = issue(&store, &app, &app.join("codex/config.toml"), Family::Codex).unwrap();
    assert!(a.id != b.id && a.secret != b.secret);
    assert!(authorize(&store, &app, &a.header(), "/v1/logs") == Some(Family::Claude));
    assert!(authorize(&store, &other, &a.header(), "/v1/logs").is_none());
    assert!(authorize(&store, &app, &a.header(), "/v1/traces").is_none());
    assert!(authorize(&store, &app, &a.header(), "/v1/traces/supplemental").is_none());
    let mut wrong = a.header();
    wrong.pop();
    wrong.push(if a.secret.ends_with('a') { 'b' } else { 'a' });
    assert!(authorize(&store, &app, &wrong, "/v1/logs").is_none());
    for header in [
        "",
        "Bearer ../target",
        "Basic abc",
        "Bearer abc.def",
        "Bearer receiver-fixture",
    ] {
        assert!(authorize(&store, &app, header, "/v1/logs").is_none());
    }
    revoke(&store, &a).unwrap();
    assert!(authorize(&store, &app, &a.header(), "/v1/logs").is_none());
    assert!(authorize(&store, &app, &b.header(), "/v1/logs") == Some(Family::Codex));
    revoke(&store, &a).unwrap();
    revoke(&store, &b).unwrap();
    assert!(store.values.lock().unwrap().is_empty());
    std::fs::remove_dir_all(app).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}
#[test]
fn unreadable_or_corrupt_credentials_fail_closed_and_cannot_delete_other_entries() {
    let store = MemoryStore::default();
    let app = root("corrupt");
    let a = issue(&store, &app, &app.join("settings.json"), Family::Claude).unwrap();
    store
        .values
        .lock()
        .unwrap()
        .insert(format!("{PREFIX}{}", a.id), b"invalid".to_vec());
    assert!(authorize(&store, &app, &a.header(), "/v1/logs").is_none());
    assert!(revoke(&store, &a).is_err());
    assert_eq!(store.values.lock().unwrap().len(), 1);
    std::fs::remove_dir_all(app).unwrap();
}
#[test]
fn family_filter_cannot_claim_other_client_logs() {
    assert!(Family::Claude.accepts_record(&serde_json::json!({"name":"claude_code.api_request"})));
    assert!(!Family::Claude.accepts_record(&serde_json::json!({"name":"codex.api_request"})));
    assert!(!Family::Codex.accepts_record(&serde_json::json!({"name":"unknown"})));
}

#[test]
fn credential_write_and_readback_failures_reclaim_only_the_exact_owned_value() {
    struct FailedReply {
        inner: MemoryStore,
        replace: bool,
        failed_reply: bool,
    }
    impl Store for FailedReply {
        fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
            self.inner.random(bytes)
        }
        fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
            self.inner.read(name)
        }
        fn delete(&self, name: &str) -> Result<(), String> {
            self.inner.delete(name)
        }
        fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
            self.inner.write(
                name,
                if self.replace {
                    b"external-replacement"
                } else {
                    bytes
                },
            )?;
            if self.failed_reply {
                Err("credential_store_unavailable".into())
            } else {
                Ok(())
            }
        }
    }
    let app = root("lost-write-reply");
    for (replace, failed_reply) in [(false, true), (true, true), (true, false)] {
        let store = FailedReply {
            inner: MemoryStore::default(),
            replace,
            failed_reply,
        };
        assert!(issue(&store, &app, &app.join("config.json"), Family::Claude).is_err());
        assert_eq!(
            store.inner.values.lock().unwrap().len(),
            usize::from(replace)
        );
    }
    std::fs::remove_dir_all(app).unwrap();
}

#[test]
fn post_write_visibility_wait_is_bounded_and_never_retries_errors_or_replacements() {
    let mut calls = 0;
    let value = read_visible_after_write(|| {
        calls += 1;
        Ok((calls == 3).then(|| b"owned".to_vec()))
    })
    .unwrap();
    assert_eq!(value.as_deref(), Some(b"owned".as_slice()));
    assert_eq!(calls, 3);
    calls = 0;
    assert!(read_visible_after_write(|| {
        calls += 1;
        Ok(None)
    })
    .unwrap()
    .is_none());
    assert_eq!(calls, 6);
    for replacement in [false, true] {
        calls = 0;
        let value = read_visible_after_write(|| {
            calls += 1;
            if replacement {
                Ok(Some(b"foreign".to_vec()))
            } else {
                Err("credential_store_unavailable".into())
            }
        });
        assert_eq!(calls, 1);
        if replacement {
            assert_eq!(value.unwrap().as_deref(), Some(b"foreign".as_slice()));
        } else {
            assert!(value.is_err());
        }
    }
}

#[test]
fn revocation_rechecks_missing_values_and_confirms_delete_without_removing_replacements() {
    struct Delayed {
        inner: MemoryStore,
        missing: AtomicUsize,
        deletes: AtomicUsize,
    }
    impl Store for Delayed {
        fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
            self.inner.random(bytes)
        }
        fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
            if self
                .missing
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                Ok(None)
            } else {
                self.inner.read(name)
            }
        }
        fn read_for_mutation(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
            read_visible_after_write(|| self.read(name))
        }
        fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
            self.inner.write(name, bytes)
        }
        fn delete(&self, name: &str) -> Result<(), String> {
            self.deletes.fetch_add(1, Ordering::SeqCst);
            self.inner.delete(name)
        }
    }
    let store = Delayed {
        inner: MemoryStore::default(),
        missing: AtomicUsize::new(0),
        deletes: AtomicUsize::new(0),
    };
    let app = root("revoke-visible");
    let binding = issue(&store, &app, &app.join("config.json"), Family::Claude).unwrap();
    store.missing.store(2, Ordering::SeqCst);
    revoke(&store, &binding).unwrap();
    assert!(store.inner.values.lock().unwrap().is_empty());
    assert_eq!(store.deletes.load(Ordering::SeqCst), 1);
    let foreign = issue(&store, &app, &app.join("other.json"), Family::Claude).unwrap();
    store.inner.values.lock().unwrap().insert(
        format!("{PREFIX}{}", foreign.id),
        b"external-replacement".to_vec(),
    );
    assert!(revoke(&store, &foreign).is_err());
    assert_eq!(store.deletes.load(Ordering::SeqCst), 1);
    let mut reads = 0;
    assert!(read_absent_after_delete(
        || {
            reads += 1;
            Ok((reads < 3).then(|| b"owned".to_vec()))
        },
        b"owned"
    )
    .unwrap());
    assert_eq!(reads, 3);
    reads = 0;
    assert!(!read_absent_after_delete(
        || {
            reads += 1;
            Ok(Some(b"owned".to_vec()))
        },
        b"owned"
    )
    .unwrap());
    assert_eq!(reads, 6);
    reads = 0;
    assert!(!read_absent_after_delete(
        || {
            reads += 1;
            Ok(Some(b"foreign".to_vec()))
        },
        b"owned"
    )
    .unwrap());
    assert_eq!(reads, 1);
    assert!(
        read_absent_after_delete(|| Err("credential_store_unavailable".into()), b"owned").is_err()
    );
    std::fs::remove_dir_all(app).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires isolated D-Bus/keyring; use npm run test:credentials:linux"]
fn native_linux_vault_unavailable() {
    assert!(!available());
    assert!(SystemStore
        .read("llm-usage/otel/v1/capability-check")
        .is_err());
    assert!(SystemStore
        .write("llm-usage/otel/v1/unavailable-fixture", b"synthetic")
        .is_err());
    let app = root("unavailable-vault");
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    assert_eq!(
        crate::otel_receiver::ensure_started_owned(port, app.join("otel")).unwrap_err(),
        "credential_store_unavailable"
    );
    let _unbound = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    assert!(!app.join("otel").exists());
    std::fs::remove_dir_all(app).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "locks only a disposable keyring; use npm run test:credentials:linux"]
fn native_linux_locked_vault() {
    assert_eq!(
        std::env::var("LLM_USAGE_ISOLATED_VAULT").as_deref(),
        Ok("1")
    );
    super::linux::run(async {
        let service =
            secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        service.get_default_collection().await?.lock().await
    })
    .unwrap();
    assert!(!available());
    assert!(SystemStore
        .read("llm-usage/otel/v1/capability-check")
        .is_err());
    assert!(SystemStore
        .write("llm-usage/otel/v1/locked-fixture", b"synthetic")
        .is_err());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "creates duplicate owned items in a disposable keyring; use npm run test:credentials:linux"]
fn native_linux_ambiguous_vault() {
    assert_eq!(
        std::env::var("LLM_USAGE_ISOLATED_VAULT").as_deref(),
        Ok("1")
    );
    super::linux::run(async {
        let service =
            secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        let collection = service.get_default_collection().await?;
        let name = "llm-usage/otel/v1/duplicate-fixture";
        let attributes = || {
            std::collections::HashMap::from([
                ("application", "org.owent.llm-usage.otel.v1"),
                ("target", name),
            ])
        };
        let a = collection
            .create_item(
                "owned synthetic duplicate",
                attributes(),
                b"one",
                false,
                "text/plain",
            )
            .await?;
        let b = collection
            .create_item(
                "owned synthetic duplicate",
                attributes(),
                b"two",
                false,
                "text/plain",
            )
            .await?;
        assert!(SystemStore.read(name).is_err());
        assert!(SystemStore.delete(name).is_err());
        assert_eq!(service.search_items(attributes()).await?.unlocked.len(), 2);
        a.delete().await?;
        b.delete().await?;
        assert!(SystemStore.read(name).unwrap().is_none());
        // A unique session-only copy is also unusable: it would disappear on
        // logout and must not silently satisfy a persisted exporter binding.
        let session = service.get_collection_by_alias("session").await?;
        let transient = session
            .create_item(
                "owned transient fixture",
                attributes(),
                b"temporary",
                false,
                "text/plain",
            )
            .await?;
        assert!(SystemStore.read(name).is_err());
        transient.delete().await?;
        Ok(())
    })
    .unwrap();
}
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "native credential store; run explicitly, creates and precisely deletes owned random entries"]
fn native_credential_store_roundtrip() {
    let app = root("native-vault");
    let binding = issue(
        &SystemStore,
        &app,
        &app.join("synthetic/config.toml"),
        Family::Codex,
    )
    .unwrap();
    struct Cleanup(Binding);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = revoke(&SystemStore, &self.0);
        }
    }
    let cleanup = Cleanup(binding);
    let other =
        Cleanup(issue(&SystemStore, &app, &app.join("other.json"), Family::Claude).unwrap());
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs") == Some(Family::Codex));
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/traces").is_none());
    // A separate process must read and revoke the persisted entry. Transfer the
    // synthetic credential only over stdin, never argv, environment or a file.
    use std::io::Write;
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "receiver_auth::tests::native_credential_child",
            "--ignored",
            "--nocapture",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cleanup.0).unwrap())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "native credential child failed");
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .contains("native credential child processed binding"),
        "native credential child did not process its binding"
    );
    if authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs").is_some() {
        eprintln!(
            "native child diagnostics: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        for delay in [10, 50, 100] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            eprintln!(
                "native credential revoke probe: delay_ms={delay}; present={}",
                authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs").is_some()
            );
        }
        panic!("credential remains after cross-process revocation");
    }
    assert!(authorize(&SystemStore, &app, &other.0.header(), "/v1/logs") == Some(Family::Claude));
    assert!(authorize(&SystemStore, &app, &cleanup.0.header(), "/v1/logs").is_none());
    assert!(SystemStore
        .read(&format!("{PREFIX}{}", cleanup.0.id))
        .unwrap()
        .is_none());
    drop(cleanup);
    revoke(&SystemStore, &other.0).unwrap();
    drop(other);
    std::fs::remove_dir_all(app).unwrap();
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "native vault and real loopback HTTP; creates and precisely reclaims owned credentials"]
fn native_credential_http_revocation() {
    use std::io::{Read, Write};
    let app = root("native-http");
    let out = app.join("otel");
    struct Cleanup {
        port: Option<u16>,
        out: std::path::PathBuf,
        bindings: Vec<Binding>,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Some(port) = self.port {
                crate::otel_receiver::stop_owned(port, &self.out);
            }
            for binding in &self.bindings {
                let _ = revoke(&SystemStore, binding);
            }
        }
    }
    // Arm cleanup before the first write, including a later issue/bind failure.
    let mut cleanup = Cleanup {
        port: None,
        out: out.clone(),
        bindings: Vec::new(),
    };
    cleanup
        .bindings
        .push(issue(&SystemStore, &app, &app.join("a.json"), Family::Claude).unwrap());
    cleanup
        .bindings
        .push(issue(&SystemStore, &app, &app.join("b.json"), Family::Claude).unwrap());
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    cleanup.port = Some(port);
    assert!(crate::otel_receiver::ensure_started_owned(port, out).unwrap());
    let request = |header: &str, body: &[u8]| {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(6)))
            .unwrap();
        write!(stream,"POST /v1/logs HTTP/1.1\r\nAuthorization: {header}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",body.len()).unwrap();
        stream.write_all(body).unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            response.push(byte[0]);
        }
        response
    };
    assert!(request("", b"").starts_with(b"HTTP/1.1 401"));
    let payload=br#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{"eventName":"claude_code.api_request","attributes":[{"key":"input_tokens","value":{"intValue":"42"}}]}]}]}]}"#;
    assert!(request(&cleanup.bindings[0].header(), payload).starts_with(b"HTTP/1.1 200"));
    let file = app.join("telemetry/otlp-logs.jsonl");
    let before = std::fs::read_to_string(&file).unwrap();
    assert_eq!(before.lines().count(), 1);
    revoke(&SystemStore, &cleanup.bindings[0]).unwrap();
    assert!(request(&cleanup.bindings[0].header(), payload).starts_with(b"HTTP/1.1 401"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    assert!(request(&cleanup.bindings[1].header(), payload).starts_with(b"HTTP/1.1 200"));
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 2);
    drop(cleanup);
    let _unbound = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    std::fs::remove_dir_all(app).unwrap();
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "helper for native_credential_store_roundtrip; synthetic binding arrives only on stdin"]
fn native_credential_child() {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(16_384)
        .read_to_end(&mut bytes)
        .unwrap();
    // Direct explicit --ignored runs have no parent input and need no vault writes.
    if bytes.is_empty() {
        return;
    }
    let binding: Binding = serde_json::from_slice(&bytes).unwrap();
    assert!(
        authorize(
            &SystemStore,
            Path::new(&binding.app),
            &binding.header(),
            "/v1/logs"
        ) == Some(binding.family)
    );
    revoke(&SystemStore, &binding).unwrap();
    println!("native credential child processed binding");
}
