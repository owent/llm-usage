use super::*;

#[test]
fn jsonc_insertion_uses_one_newline_and_preserves_comments_and_crlf() {
    for newline in ["\n", "\r\n"] {
        let text = format!(
            "{{{newline}  // retained{newline}  \"editor.fontSize\": 14{newline}}}{newline}"
        );
        let changes = vec![
            change(&["github.copilot.chat.otel.enabled"], json!(true)),
            change(&["github.copilot.chat.otel.exporterType"], json!("file")),
            change(
                &["github.copilot.chat.otel.outfile"],
                json!("C:/local/events.jsonl"),
            ),
        ];
        let output = edit::merge_json(&text, &changes).unwrap();
        assert!(!output.contains(&format!("{newline}{newline}")));
        assert!(output.contains(&format!("// retained{newline}")));
        assert_eq!(edit::merge_json(&output, &changes).unwrap(), output);
        assert_eq!(edit::json_value(&output).unwrap()["editor.fontSize"], 14);
    }
}

#[test]
fn exporter_evidence_is_automatic_cached_and_does_not_override_configuration() {
    let f = Fixture::new();
    f.copilot();
    let config = f.ctx.config.join("Code/User/settings.json");
    let output = f.root.join("native-export.jsonl");
    f.write(
        &config,
        &json!({"github.copilot.chat.otel.enabled":true,
        "github.copilot.chat.otel.exporterType":"file","github.copilot.chat.otel.outfile":output})
        .to_string(),
    );
    let mut row = inspect(
        discover(&f.ctx)
            .into_iter()
            .find(|r| r.dto.id == "copilot-vscode")
            .unwrap(),
    );
    evidence::verify(&mut row);
    assert_eq!(row.dto.verification, "waiting");
    let span = json!({"name":"chat gpt-5.4","kind":2,"traceId":"synthetic-trace","spanId":"synthetic-span",
        "startTime":[1790899200,0],"resource":{"attributes":{"service.name":"copilot-chat"}},
        "attributes":{"copilot_chat.chat_session_id":"synthetic-session","gen_ai.request.model":"gpt-5.4","gen_ai.usage.input_tokens":100,"gen_ai.usage.output_tokens":20}});
    f.write(&output, &format!("{span}\n"));
    evidence::verify(&mut row);
    assert_eq!(
        (
            &*row.dto.status,
            &*row.dto.verification,
            row.dto.verified_records
        ),
        ("configured", "verified", 1)
    );
    // Identical bytes reuse the cached check; file changes invalidate it.
    evidence::verify(&mut row);
    assert_eq!(row.dto.verified_records, 1);
    f.write(&output, "{\"body\":\"unrelated log\"}\n");
    row.dto.verified_records = 0;
    evidence::verify(&mut row);
    assert_eq!(row.dto.verification, "unrecognized");
    f.write(&output, &format!("{span}\n"));
    row.dto.status = "blocked".into();
    row.dto.reason = "managed_policy".into();
    evidence::verify(&mut row);
    assert_eq!(
        (&*row.dto.status, &*row.dto.verification),
        ("blocked", "verified")
    );
}

#[test]
fn shared_output_does_not_prove_which_profile_is_exporting() {
    let f = Fixture::new();
    f.copilot();
    let output = f.root.join("shared.jsonl");
    let config = json!({"github.copilot.chat.otel.enabled":true,
        "github.copilot.chat.otel.exporterType":"file","github.copilot.chat.otel.outfile":output})
    .to_string();
    f.write(&f.ctx.config.join("Code/User/settings.json"), &config);
    f.write(
        &f.ctx.config.join("Code/User/profiles/work/settings.json"),
        &config,
    );
    let span = json!({"name":"chat gpt-5.4","kind":2,"traceId":"synthetic","spanId":"shared",
        "startTime":[1790899200,0],"resource":{"attributes":{"service.name":"copilot-chat"}},
        "attributes":{"copilot_chat.chat_session_id":"synthetic","gen_ai.usage.input_tokens":100}});
    f.write(&output, &format!("{span}\n"));
    let targets = evidence::verify_all(discover(&f.ctx).into_iter().map(inspect).collect());
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|row| row.dto.status == "configured"
        && row.dto.verification == "external"
        && row.dto.verified_records == 0));
}

struct Fixture {
    ctx: Context,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/telemetry-setup-tests")
            .join(nonce());
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let ctx = Context {
            home: root.join("home"),
            config: root.join("config"),
            data: root.join("local"),
            app: root.join("app"),
            env: Default::default(),
            port: 4318,
            command_dirs: vec![root.join("bin")],
            external_manifests: vec![],
            policy_files: vec![],
        };
        Self { ctx, root }
    }
    fn write(&self, path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn install(&self, command: &str) {
        let suffix = if cfg!(windows) { ".cmd" } else { "" };
        self.write(
            &self.root.join("bin").join(format!("{command}{suffix}")),
            "fixture launcher, never executed",
        );
    }
    fn copilot(&self) {
        let keys = [
            "enabled",
            "exporterType",
            "outfile",
            "captureContent",
            "captureIdentity",
        ]
        .map(|s| format!("github.copilot.chat.otel.{s}"));
        let properties: serde_json::Map<String, Value> =
            keys.into_iter().map(|k| (k, json!({}))).collect();
        self.write(&self.ctx.data.join("Programs/Microsoft VS Code/version-dir/resources/app/extensions/copilot/package.json"),&json!({"name":"copilot-chat","publisher":"GitHub","version":"0.68.0","contributes":{"configuration":[{"properties":properties}]}}).to_string());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn discovery_does_not_create_user_files_or_installations() {
    let f = Fixture::new();
    assert!(discover(&f.ctx).is_empty());
    assert!(!f.ctx.home.exists());
    assert!(!f.ctx.app.exists());
}

#[test]
fn leftover_user_directories_and_extension_cache_do_not_prove_installation() {
    let f = Fixture::new();
    for name in [
        ".gemini",
        ".qwen",
        ".claude",
        ".codex",
        ".copilot",
        ".codebuddy",
    ] {
        f.write(&f.ctx.home.join(name).join("settings.json"), "{}");
    }
    f.write(
        &f.ctx
            .config
            .join("Code/User/globalStorage/github.copilot-chat/state.json"),
        "{}",
    );
    f.write(
        &f.ctx
            .config
            .join("Code/User/profiles/leftover/settings.json"),
        "{}",
    );
    f.write(
        &f.ctx
            .home
            .join(".vscode/extensions/github.copilot-chat-leftover/package.json"),
        "{\"name\":\"copilot-chat\",\"publisher\":\"GitHub\"}",
    );
    assert!(discover(&f.ctx).is_empty());
    assert!(!f.ctx.app.exists());
}

#[test]
fn standalone_copilot_extension_requires_its_editor_launcher() {
    let f = Fixture::new();
    f.write(
        &f.ctx
            .home
            .join(".vscode/extensions/github.copilot-chat-installed/package.json"),
        "{\"name\":\"copilot-chat\",\"publisher\":\"GitHub\"}",
    );
    assert!(discover(&f.ctx).is_empty());
    f.install("code");
    let rows = discover(&f.ctx);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        inspect(rows.into_iter().next().unwrap()).dto.reason,
        "unsupported_version"
    );
}

#[test]
fn similarly_named_extension_from_another_publisher_is_not_copilot() {
    let f = Fixture::new();
    f.copilot();
    let path = f.ctx.data.join(
        "Programs/Microsoft VS Code/version-dir/resources/app/extensions/copilot/package.json",
    );
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["publisher"] = json!("another-publisher");
    f.write(&path, &manifest.to_string());
    assert!(discover(&f.ctx).is_empty());
}

#[test]
fn bundled_version_directory_is_detected_and_profiles_are_user_scoped() {
    let f = Fixture::new();
    f.copilot();
    f.write(
        &f.ctx.config.join("Code/User/profiles/work/settings.json"),
        "{}",
    );
    let rows = discover(&f.ctx);
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|t| t.path.starts_with(f.ctx.config.join("Code/User"))));
    assert!(rows.into_iter().map(inspect).all(|t| t.dto.configurable));
    assert!(!f.ctx.config.join("Code/User/settings.json").exists());
}

#[test]
fn sync_exclusions_merge_without_erasing_user_settings() {
    let f = Fixture::new();
    f.copilot();
    let path = f.ctx.config.join("Code/User/settings.json");
    f.write(
        &path,
        "{\"settingsSync.ignoredSettings\":[\"some.setting\"],\"editor.fontSize\":16}",
    );
    let row = inspect(discover(&f.ctx).remove(0));
    assert!(row.dto.configurable);
    let bytes = readable(&path).unwrap().unwrap();
    let output = merge(&row, std::str::from_utf8(&bytes).unwrap()).unwrap();
    let value = edit::json_value(&output).unwrap();
    assert_eq!(value["editor.fontSize"], 16);
    let ignored = value["settingsSync.ignoredSettings"].as_array().unwrap();
    assert!(ignored.contains(&json!("some.setting")));
    assert!(ignored.contains(&json!("github.copilot.chat.otel.outfile")));
}

#[test]
fn explicit_sync_inclusion_and_telemetry_kill_switch_block_writes() {
    let f = Fixture::new();
    f.copilot();
    let path = f.ctx.config.join("Code/User/settings.json");
    for (text, reason) in [
        (
            "{\"settingsSync.ignoredSettings\":[\"-github.copilot.chat.otel.outfile\"]}",
            "sync_conflict",
        ),
        (
            "{\"telemetry.telemetryLevel\":\"off\"}",
            "telemetry_disabled",
        ),
    ] {
        f.write(&path, text);
        let row = inspect(discover(&f.ctx).remove(0));
        assert_eq!(row.dto.reason, reason);
        assert!(!row.dto.configurable);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn existing_collector_is_preserved_and_not_replaced_by_local_defaults() {
    let f = Fixture::new();
    f.copilot();
    f.write(&f.ctx.config.join("Code/User/settings.json"),"{\"github.copilot.chat.otel.enabled\":true,\"github.copilot.chat.otel.exporterType\":\"otlp-http\",\"github.copilot.chat.otel.otlpEndpoint\":\"https://collector.example\"}");
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.dto.status, "configured");
    assert!(!row.dto.configurable);
}

#[test]
fn codex_http_exporter_merges_leaf_keys_in_tables_and_inline_tables() {
    let f = Fixture::new();
    f.install("codex");
    let path = f.ctx.home.join(".codex/config.toml");
    for text in [
        "# keep\n[otel.exporter.otlp-http]\nprotocol = 'binary'\nheaders = { test = 'opaque' }\nfuture_option = 42 # user option\n",
        "# keep\n[otel]\nexporter = { otlp-http = { protocol = 'binary', headers = { test = 'opaque' }, future_option = 42 } }\n",
        "# keep\notel = { exporter = { otlp-http = { protocol = 'binary', headers = { test = 'opaque' }, future_option = 42 } } }\n",
    ] {
        f.write(&path, text);
        let target = inspect(discover(&f.ctx).remove(0));
        assert!(target.dto.configurable, "{}", target.dto.reason);
        let merged = merge(&target, text).unwrap();
        assert!(merged.contains("# keep"));
        let value = edit::toml_value(&merged).unwrap();
        let http = &value["otel"]["exporter"]["otlp-http"];
        assert_eq!(http["headers"]["test"], "opaque");
        assert_eq!(http["future_option"], 42);
        assert_eq!(http["protocol"], "json");
        assert_eq!(http["endpoint"], "http://127.0.0.1:4318/v1/logs");
        assert_eq!(merge(&target, &merged).unwrap(), merged);
        let edited = format!("{merged}\n[other]\nuser_edit = true\n");
        let (restored, conflicts) = edit::restore_toml(&edited, &edit::toml_value(text).unwrap(), &target.changes).unwrap();
        assert!(conflicts.is_empty());
        let restored = edit::toml_value(&restored).unwrap();
        assert_eq!(restored["other"]["user_edit"], true);
        assert_eq!(restored["otel"]["exporter"], edit::toml_value(text).unwrap()["otel"]["exporter"]);
    }
}

#[test]
fn codex_conflicting_or_unrecognized_exporter_shapes_are_preserved() {
    let f = Fixture::new();
    f.install("codex");
    let path = f.ctx.home.join(".codex/config.toml");
    for text in [
        "[otel]\nexporter = { custom = {} }\n",
        "[otel]\nexporter = { otlp-http = {}, otlp-grpc = {} }\n",
        "[otel]\nexporter = { otlp-http = false }\n",
        "[otel]\nexporter = { otlp-grpc = { headers = { test = 'opaque' } } }\n",
    ] {
        f.write(&path, text);
        let target = inspect(discover(&f.ctx).remove(0));
        assert!(!target.dto.configurable);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn missing_extension_schema_fails_closed_instead_of_guessing_version_support() {
    let f = Fixture::new();
    f.write(
        &f.ctx
            .data
            .join("Programs/Microsoft VS Code/resources/app/extensions/copilot/package.json"),
        "{\"name\":\"copilot-chat\",\"publisher\":\"GitHub\"}",
    );
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.dto.reason, "unsupported_version");
    assert!(!row.dto.configurable);
}

#[test]
fn environment_override_blocks_only_affected_client() {
    let mut f = Fixture::new();
    f.install("gemini");
    f.install("qwen");
    std::fs::create_dir_all(f.ctx.home.join(".qwen")).unwrap();
    std::fs::create_dir_all(f.ctx.home.join(".gemini")).unwrap();
    f.ctx
        .env
        .insert("QWEN_TELEMETRY_OUTFILE".into(), "not-returned".into());
    let rows = discover(&f.ctx)
        .into_iter()
        .map(inspect)
        .collect::<Vec<_>>();
    assert!(
        !rows
            .iter()
            .find(|t| t.dto.id == "qwen")
            .unwrap()
            .dto
            .configurable
    );
    assert!(
        rows.iter()
            .find(|t| t.dto.id == "gemini")
            .unwrap()
            .dto
            .configurable
    );
    assert!(
        !serde_json::to_string(&rows.iter().map(|t| &t.dto).collect::<Vec<_>>())
            .unwrap()
            .contains("not-returned")
    );
}

#[test]
fn custom_codex_home_and_existing_toml_credentials_are_retained() {
    let mut f = Fixture::new();
    f.install("codex");
    let dir = f.ctx.home.join("custom-codex");
    std::fs::create_dir_all(&dir).unwrap();
    f.ctx
        .env
        .insert("CODEX_HOME".into(), dir.display().to_string());
    f.write(&dir.join("config.toml"),"model = 'custom'\n[model_providers.custom]\nenv_key = 'OPAQUE'\n[otel]\nexporter = 'none'\n");
    let row = inspect(discover(&f.ctx).remove(0));
    assert!(row.dto.configurable);
    assert!(row.receiver);
    let output = merge(&row, &std::fs::read_to_string(&row.path).unwrap()).unwrap();
    assert!(output.contains("env_key = 'OPAQUE'"));
    assert_eq!(
        edit::toml_value(&output).unwrap()["otel"]["exporter"]["otlp-http"]["endpoint"],
        "http://127.0.0.1:4318/v1/logs"
    );
}

#[test]
fn malformed_and_oversized_configs_are_not_exposed_or_overwritten() {
    let f = Fixture::new();
    f.install("qwen");
    std::fs::create_dir_all(f.ctx.home.join(".qwen")).unwrap();
    let path = f.ctx.home.join(".qwen/settings.json");
    for text in ["{\"secret\":\"SENSITIVE\",broken", "{\"telemetry\":false}"] {
        f.write(&path, text);
        let row = inspect(discover(&f.ctx).remove(0));
        assert!(!row.dto.configurable);
        assert!(!row.dto.reason.contains("SENSITIVE"));
    }
    std::fs::write(&path, vec![b' '; MAX_CONFIG as usize + 1]).unwrap();
    assert_eq!(
        inspect(discover(&f.ctx).remove(0)).dto.reason,
        "oversized_config"
    );
}

#[test]
fn commit_is_atomic_backs_up_exact_bytes_and_detects_concurrent_edits() {
    let f = Fixture::new();
    let path = f.ctx.home.join("settings.json");
    let backup = f.ctx.app.join("backups");
    f.write(&path, "{/* preserve */\"x\":1}");
    let before = std::fs::read(&path).unwrap();
    commit(
        &path,
        Some(&before),
        b"{\"x\":1,\"otel\":true}",
        &backup,
        "apply",
    )
    .unwrap();
    assert_eq!(std::fs::read(backup.join("apply.backup")).unwrap(), before);
    assert_eq!(
        commit(&path, Some(&before), b"{}", &backup, "stale").unwrap_err(),
        "config_changed"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\"x\":1,\"otel\":true}"
    );
    assert!(!path.parent().unwrap().join(".llm-usage-stale.tmp").exists());
}

#[test]
fn backup_failure_preserves_original_and_removes_temporary_file() {
    let f = Fixture::new();
    let path = f.ctx.home.join("settings.json");
    f.write(&path, "{}");
    let backup = f.ctx.app.join("not-directory");
    f.write(&backup, "occupied");
    assert_eq!(
        commit(&path, Some(b"{}"), b"{\"x\":1}", &backup, "failed").unwrap_err(),
        "backup_failed"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    assert!(!path
        .parent()
        .unwrap()
        .join(".llm-usage-failed.tmp")
        .exists());
}

#[test]
fn readonly_config_is_preserved() {
    let f = Fixture::new();
    let path = f.ctx.home.join("settings.json");
    f.write(&path, "{}");
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    assert_eq!(
        commit(
            &path,
            Some(b"{}"),
            b"{\"x\":1}",
            &f.ctx.app.join("backups"),
            "readonly"
        )
        .unwrap_err(),
        "read_only"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    std::fs::set_permissions(&path, original).unwrap();
}

#[test]
fn launcher_does_not_write_copilot_application_state_or_global_environment() {
    let f = Fixture::new();
    f.install("copilot");
    std::fs::create_dir_all(f.ctx.home.join(".copilot")).unwrap();
    f.write(
        &f.ctx.home.join(".copilot/config.json"),
        "{\"state\":\"opaque\"}",
    );
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.format, "launcher");
    assert!(row.path.starts_with(&f.ctx.app));
    let text = launcher(&f.ctx.app.join("path with ' quote/events.jsonl"));
    assert!(text.contains("copilot"));
    assert!(!text.contains("User'"));
    assert!(!text.contains("Machine'"));
    if cfg!(windows) {
        assert!(text.contains("finally"));
        assert!(text.contains("'' quote"));
    }
    assert_eq!(
        std::fs::read_to_string(f.ctx.home.join(".copilot/config.json")).unwrap(),
        "{\"state\":\"opaque\"}"
    );
}

#[test]
fn codebuddy_authentication_is_limited_to_its_documented_installed_version() {
    let mut f = Fixture::new();
    f.install("codebuddy");
    std::fs::create_dir_all(f.ctx.home.join(".codebuddy")).unwrap();
    let row = inspect(discover(&f.ctx).remove(0));
    assert!(!row.dto.configurable);
    assert_eq!(row.dto.reason, "unsupported_version");
    assert!(row.receiver);
    let merged = edit::json_value(
        &merge(
            &row,
            "{\"env\":{\"EXISTING\":\"keep\"},\"model\":\"custom\"}",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(merged["env"]["CODEBUDDY_CODE_ENABLE_TELEMETRY"], "1");
    assert_eq!(merged["env"]["EXISTING"], "keep");
    assert_eq!(
        merged["env"]["OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"],
        "http://127.0.0.1:4318/v1/traces/supplemental"
    );
    assert_eq!(merged["env"]["OTEL_LOG_TOOL_CONTENT"], "0");
    assert!(row.dto.output_path.contains("otlp-traces.jsonl"));
    let manifest =
        f.ctx.command_dirs[0].join("node_modules/@tencent-ai/codebuddy-code/package.json");
    for version in ["2.97.0", "2.98.0", "2.99.0", "3.0.0"] {
        f.write(
            &manifest,
            &json!({"name":"@tencent-ai/codebuddy-code","version":version}).to_string(),
        );
        let target = inspect(discover(&f.ctx).remove(0));
        assert_eq!(target.dto.configurable, version == "2.98.0");
        if version == "2.98.0" {
            assert!(merge(&target, "{}")
                .unwrap()
                .contains("OTEL_EXPORTER_OTLP_HEADERS"));
        }
    }
    // A manifest in a later PATH entry cannot verify the first launcher.
    let first = f.root.join("different-install");
    std::fs::create_dir_all(&first).unwrap();
    f.write(
        &first.join(if cfg!(windows) {
            "codebuddy.cmd"
        } else {
            "codebuddy"
        }),
        "unused",
    );
    f.ctx.command_dirs.insert(0, first);
    f.write(
        &manifest,
        &json!({"name":"@tencent-ai/codebuddy-code","version":"2.98.0"}).to_string(),
    );
    assert!(!codebuddy_auth_version(&f.ctx));
    f.ctx.command_dirs.remove(0);
    assert!(codebuddy_auth_version(&f.ctx));
}

#[test]
fn codebuddy_generic_headers_encode_bearer_and_preserve_unrelated_keys_without_preview_leaks() {
    let f = Fixture::new();
    f.install("codebuddy");
    f.write(
        &f.ctx.command_dirs[0].join("node_modules/@tencent-ai/codebuddy-code/package.json"),
        r#"{"name":"@tencent-ai/codebuddy-code","version":"2.98.0"}"#,
    );
    let path = f.ctx.home.join(".codebuddy/settings.json");
    f.write(
        &path,
        r#"{"env":{"OTEL_EXPORTER_OTLP_HEADERS":"x-private=PRIVATE_SENTINEL"}}"#,
    );
    let mut target = inspect(discover(&f.ctx).remove(0));
    assert!(target.dto.configurable);
    let value = parse(&target, readable(&path).unwrap().as_deref()).unwrap();
    assert!(!serde_json::to_string(&preview_changes(&target, &value))
        .unwrap()
        .contains("PRIVATE_SENTINEL"));
    add_authentication(&mut target, &value, "Bearer synthetic-value").unwrap();
    let output = merge(&target, &std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(output.contains("x-private=PRIVATE_SENTINEL,Authorization=Bearer%20synthetic-value"));
    let decoded = exporter_authorization(
        &edit::json_value(&output).unwrap(),
        "OTEL_EXPORTER_OTLP_HEADERS",
    )
    .unwrap();
    assert_eq!(decoded.as_deref(), Some("Bearer synthetic-value"));
    f.write(
        &path,
        r#"{"env":{"OTEL_EXPORTER_OTLP_TRACES_HEADERS":"x-private=not-verified-in-2.98"}}"#,
    );
    assert_eq!(
        inspect(discover(&f.ctx).remove(0)).dto.reason,
        "authentication_unverified"
    );
}

#[test]
fn http_preview_redacts_existing_headers_and_checking_never_creates_credentials() {
    let f = Fixture::new();
    f.install("claude");
    let path = f.ctx.home.join(".claude/settings.json");
    f.write(
        &path,
        r#"{"env":{"OTEL_EXPORTER_OTLP_LOGS_HEADERS":"x-api-key=SENSITIVE,x-tag=preserved"}}"#,
    );
    let target = inspect(discover(&f.ctx).remove(0));
    assert!(target.dto.configurable);
    let value = parse(&target, readable(&path).unwrap().as_deref()).unwrap();
    let preview = serde_json::to_string(&preview_changes(&target, &value)).unwrap();
    assert!(!preview.contains("SENSITIVE"));
    assert!(preview.contains(crate::receiver_auth::PLACEHOLDER));
    let prospective = merge(&target, &std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(prospective.contains("x-api-key=SENSITIVE,x-tag=preserved,Authorization="));
    assert_eq!(
        readable(&path).unwrap().unwrap(),
        std::fs::read(&path).unwrap()
    );
    // CI can run static inspection; the actual UI refuses setup without a native credential store.
    if !crate::receiver_auth::available() {
        let supported = inspect_supported(discover(&f.ctx).remove(0));
        assert_eq!(supported.dto.reason, "credential_store_unavailable");
        assert!(!supported.dto.configurable);
    }
}

#[test]
fn authenticated_configuration_failure_revokes_only_new_key_and_can_retry() {
    use crate::receiver_auth::{self, tests::MemoryStore, Family};
    let f = Fixture::new();
    f.install("codex");
    let path = f.ctx.home.join(".codex/config.toml");
    f.write(&path, "[otel]\nexporter='none'\n");
    let target = inspect(discover(&f.ctx).remove(0));
    let before = readable(&path).unwrap();
    let after = merge(
        &target,
        std::str::from_utf8(before.as_deref().unwrap()).unwrap(),
    )
    .unwrap()
    .into_bytes();
    let mut plan = Plan {
        target,
        before,
        after,
        created: std::time::Instant::now(),
        applied: false,
        sync: None,
        credential: None,
    };
    let store = MemoryStore::default();
    std::fs::create_dir_all(&f.ctx.app).unwrap();
    let other = receiver_auth::issue(
        &store,
        &f.ctx.app,
        &f.ctx.home.join("other/settings.json"),
        Family::Claude,
    )
    .unwrap();
    let occupied = f.ctx.app.join("occupied-backup");
    f.write(&occupied, "occupied");
    let original_after = plan.after.clone();
    let original_changes = plan.target.changes.clone();
    assert_eq!(
        authenticated_commit(&mut plan, &f.ctx.app, &occupied, "failure", &store).unwrap_err(),
        "backup_failed"
    );
    assert!(plan.credential.is_none());
    assert_eq!(plan.after, original_after);
    assert_eq!(plan.target.changes, original_changes);
    assert_eq!(readable(&path).unwrap(), plan.before);
    assert_eq!(store.values.lock().unwrap().len(), 1);
    authenticated_commit(
        &mut plan,
        &f.ctx.app,
        &f.ctx.app.join("auth-backups"),
        "success",
        &store,
    )
    .unwrap();
    let header = plan.credential.as_ref().unwrap().header();
    let saved = edit::toml_value(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(saved["otel"]["exporter"]["otlp-http"]["headers"]["Authorization"] == header);
    assert!(
        receiver_auth::authorize(&store, &f.ctx.app, &header, "/v1/logs") == Some(Family::Codex)
    );
    receiver_auth::revoke(&store, plan.credential.as_ref().unwrap()).unwrap();
    assert!(receiver_auth::authorize(&store, &f.ctx.app, &header, "/v1/logs").is_none());
    assert!(
        receiver_auth::authorize(&store, &f.ctx.app, &other.header(), "/v1/logs")
            == Some(Family::Claude)
    );
    receiver_auth::revoke(&store, &other).unwrap();
    assert!(store.values.lock().unwrap().is_empty());
}

#[test]
fn missing_vault_at_our_endpoint_requires_confirmation_and_remote_targets_keep_credentials() {
    let f = Fixture::new();
    f.install("claude");
    let path = f.ctx.home.join(".claude/settings.json");
    f.write(&path,r#"{"env":{"CLAUDE_CODE_ENABLE_TELEMETRY":"1","OTEL_LOGS_EXPORTER":"otlp","OTEL_EXPORTER_OTLP_LOGS_ENDPOINT":"http://127.0.0.1:4318/v1/logs"}}"#);
    let target = inspect(discover(&f.ctx).remove(0));
    assert_eq!(target.dto.status, "missing");
    assert!(target.dto.configurable);
    f.write(&path,r#"{"env":{"CLAUDE_CODE_ENABLE_TELEMETRY":"1","OTEL_LOGS_EXPORTER":"otlp","OTEL_EXPORTER_OTLP_LOGS_ENDPOINT":"https://example.invalid/v1/logs","OTEL_EXPORTER_OTLP_LOGS_HEADERS":"Authorization=FOREIGN"}}"#);
    let original = std::fs::read(&path).unwrap();
    let target = inspect(discover(&f.ctx).remove(0));
    assert_eq!(target.dto.status, "configured");
    assert_eq!(target.dto.verification, "external");
    assert!(!target.dto.configurable);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!serde_json::to_string(&target.dto)
        .unwrap()
        .contains("FOREIGN"));
}

#[test]
fn agent_host_uses_an_independent_verified_namespace() {
    let f = Fixture::new();
    f.copilot();
    f.write(
        &f.ctx
            .data
            .join("Programs/Microsoft VS Code/version-dir/resources/app/package.json"),
        "{\"version\":\"1.140.0\"}",
    );
    let row = inspect(
        discover(&f.ctx)
            .into_iter()
            .find(|t| t.dto.id.ends_with("-agent-host"))
            .unwrap(),
    );
    let merged = edit::json_value(&merge(&row, "{}").unwrap()).unwrap();
    assert_eq!(merged["chat.agentHost.otel.enabled"], true);
    assert!(merged.get("github.copilot.chat.otel.enabled").is_none());
    assert!(row.dto.output_path.contains("agent-host"));
}

#[test]
#[ignore = "Read-only local installation/configuration audit, explicitly run by the maintainer"]
fn local_configuration_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/telemetry-setup-evidence")
        .canonicalize()
        .unwrap();
    let ctx = Context::current(root, 4318).unwrap();
    for row in discover(&ctx).into_iter().map(inspect) {
        println!(
            "{} status={} reason={} kind={} configurable={}",
            row.dto.id, row.dto.status, row.dto.reason, row.dto.kind, row.dto.configurable
        );
    }
}

#[test]
#[ignore = "Read-only real export verification; print whitelisted state and counts only"]
fn local_export_verification() {
    let app = PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("llm-usage-desktop");
    let ctx = Context::current(app, 4318).unwrap();
    for row in evidence::verify_all(discover(&ctx).into_iter().map(inspect).collect()) {
        let client = if row.dto.id.contains("-profile-") {
            "copilot-vscode profile"
        } else {
            row.dto.id.as_str()
        };
        println!(
            "{} configuration={} reason={} evidence={} records={}",
            client, row.dto.status, row.dto.reason, row.dto.verification, row.dto.verified_records
        );
    }
}

#[test]
fn installed_cli_without_a_config_directory_still_has_a_setup_entry() {
    let f = Fixture::new();
    f.install("gemini");
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.dto.id, "gemini");
    assert!(row.dto.configurable);
    assert!(!f.ctx.home.join(".gemini").exists());
}

#[test]
fn gemini_home_is_a_replacement_home_but_qwen_home_is_the_config_directory() {
    let mut f = Fixture::new();
    f.install("gemini");
    f.install("qwen");
    let home = f.root.join("replacement-home");
    f.ctx
        .env
        .insert("GEMINI_CLI_HOME".into(), home.display().to_string());
    f.ctx.env.insert("QWEN_HOME".into(), "~/custom-qwen".into());
    f.write(&home.join(".gemini/settings.json"), "{}");
    f.write(&f.ctx.home.join("custom-qwen/settings.json"), "{}");
    let rows = discover(&f.ctx);
    assert_eq!(
        rows.iter().find(|t| t.dto.id == "gemini").unwrap().path,
        home.join(".gemini/settings.json")
    );
    assert_eq!(
        rows.iter().find(|t| t.dto.id == "qwen").unwrap().path,
        f.ctx.home.join("custom-qwen/settings.json")
    );
}

#[test]
fn profile_sync_exclusions_are_written_to_application_settings_in_a_transaction() {
    let f = Fixture::new();
    f.copilot();
    let profile = f.ctx.config.join("Code/User/profiles/work/settings.json");
    let application = f.ctx.config.join("Code/User/settings.json");
    f.write(&profile, "{/* profile */\"editor.fontSize\":16}");
    f.write(
        &application,
        "{/* app */\"settingsSync.ignoredSettings\":[\"existing.setting\"],\"window.zoomLevel\":2}",
    );
    let row = inspect(
        discover(&f.ctx)
            .into_iter()
            .find(|t| t.dto.id.ends_with("-profile-work"))
            .unwrap(),
    );
    assert!(row.dto.configurable);
    assert_eq!(row.sync_path.as_ref(), Some(&application));
    let before = readable(&profile).unwrap();
    let after = merge(
        &row,
        std::str::from_utf8(before.as_deref().unwrap()).unwrap(),
    )
    .unwrap()
    .into_bytes();
    let sync = prepare_sync(&row).unwrap();
    let plan = Plan {
        target: row,
        before,
        after,
        created: std::time::Instant::now(),
        applied: false,
        sync,
        credential: None,
    };
    commit_plan(&plan, &f.ctx.app.join("backups"), "profile-success").unwrap();
    let profile_value = edit::json_value(&std::fs::read_to_string(&profile).unwrap()).unwrap();
    let app_value = edit::json_value(&std::fs::read_to_string(&application).unwrap()).unwrap();
    assert!(profile_value.get("settingsSync.ignoredSettings").is_none());
    assert_eq!(profile_value["editor.fontSize"], 16);
    assert_eq!(app_value["window.zoomLevel"], 2);
    let ignored = app_value["settingsSync.ignoredSettings"]
        .as_array()
        .unwrap();
    assert!(ignored.contains(&json!("existing.setting")));
    assert!(ignored.contains(&json!("github.copilot.chat.otel.outfile")));
}

#[test]
fn failed_profile_write_rolls_back_application_sync_edits() {
    let f = Fixture::new();
    f.copilot();
    let profile = f.ctx.config.join("Code/User/profiles/work/settings.json");
    let application = f.ctx.config.join("Code/User/settings.json");
    f.write(&profile, "{}");
    f.write(&application, "{/* retained */\"editor.fontSize\":14}");
    let row = inspect(
        discover(&f.ctx)
            .into_iter()
            .find(|t| t.dto.id.ends_with("-profile-work"))
            .unwrap(),
    );
    let before = readable(&profile).unwrap();
    let after = merge(&row, "{}").unwrap().into_bytes();
    let sync = prepare_sync(&row).unwrap();
    let original_application = std::fs::read(&application).unwrap();
    let permissions = std::fs::metadata(&profile).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&profile, readonly).unwrap();
    let plan = Plan {
        target: row,
        before,
        after,
        created: std::time::Instant::now(),
        applied: false,
        sync,
        credential: None,
    };
    assert_eq!(
        commit_plan(&plan, &f.ctx.app.join("backups"), "profile-failed").unwrap_err(),
        "read_only"
    );
    assert_eq!(std::fs::read(&profile).unwrap(), b"{}");
    assert_eq!(std::fs::read(&application).unwrap(), original_application);
    std::fs::set_permissions(&profile, permissions).unwrap();
}

#[test]
fn disabled_existing_destination_is_preserved_and_outfile_alone_is_a_file_exporter() {
    let f = Fixture::new();
    f.copilot();
    let path = f.ctx.config.join("Code/User/settings.json");
    f.write(
        &path,
        "{\"github.copilot.chat.otel.outfile\":\"C:/user-chosen/events.jsonl\"}",
    );
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.dto.reason, "existing_destination");
    assert!(!row.dto.configurable);
    f.write(&path,"{\"github.copilot.chat.otel.enabled\":true,\"github.copilot.chat.otel.outfile\":\"C:/user-chosen/events.jsonl\"}");
    let row = inspect(discover(&f.ctx).remove(0));
    assert_eq!(row.dto.status, "configured");
    assert!(!row.dto.configurable);
}

#[test]
fn only_verified_app_owned_exporter_families_are_automatically_discovered() {
    let f = Fixture::new();
    for id in [
        "copilot-vscode",
        "copilot-vscode-insiders-profile-test-agent-host",
        "gemini",
        "qwen",
        "copilot-cli",
    ] {
        f.write(
            &f.ctx.app.join("telemetry").join(id).join("events.jsonl"),
            "{}\n",
        );
    }
    let roots = verified_usage_roots(&f.ctx.app);
    assert_eq!(roots.len(), 3);
    assert!(roots.iter().all(|p| p
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("copilot-vscode")
        || p.file_name().unwrap() == "qwen"));
}

#[test]
fn genuine_qwen_sdk_file_is_checked_read_only_and_unknown_version_is_not_certified() {
    let f = Fixture::new();
    f.install("qwen");
    let path = f.ctx.home.join(".qwen/settings.json");
    f.write(&path, "{}");
    let raw =
        include_str!("../../crates/core/tests/fixtures/otel/real-qwen-0.25.0-sdk/telemetry.json");
    let output = f.ctx.output("qwen");
    f.write(&output, raw);
    let before = std::fs::read(&path).unwrap();
    let rows = evidence::verify_all(discover(&f.ctx).into_iter().map(inspect).collect());
    let row = rows.iter().find(|t| t.dto.id == "qwen").unwrap();
    assert_eq!(row.dto.verification, "verified");
    assert_eq!(row.dto.verified_records, 2);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_to_string(&output).unwrap(), raw);
    f.write(&output, &raw.replace("0.25.0", "0.999.0"));
    let rows = evidence::verify_all(discover(&f.ctx).into_iter().map(inspect).collect());
    assert_ne!(
        rows.iter()
            .find(|t| t.dto.id == "qwen")
            .unwrap()
            .dto
            .verification,
        "verified"
    );
}

#[cfg(unix)]
#[test]
fn symlink_config_is_rejected() {
    let f = Fixture::new();
    let external = f.ctx.home.join("target");
    f.write(&external, "{}");
    let link = f.ctx.home.join("settings.json");
    std::os::unix::fs::symlink(&external, &link).unwrap();
    assert_eq!(readable(&link).unwrap_err(), "unsafe_path");
}
