//! User-level telemetry discovery and opt-in configuration. Checking never writes.
//! Copilot file outputs use explicit session/day carrier selection before statistics.
mod edit;
mod evidence;

use crate::app_state::{save_settings, AppState};
use edit::{lookup, Change};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_CONFIG: u64 = 2 * 1024 * 1024;
const COPILOT_DOC: &str = "https://code.visualstudio.com/docs/agents/guides/monitoring-agents";

/// Bounded discovery of app-owned, verified Copilot/Qwen exporters. Never read arbitrary
/// settings paths or promote other clients' supplemental logs to usage statistics.
pub(crate) fn verified_usage_roots(app: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(app.join("telemetry"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .take(128)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            if !name.starts_with("copilot-vscode")
                && !name.starts_with("copilot-agent-host")
                && name != "qwen"
            {
                return None;
            }
            let root = entry.path();
            let metadata = std::fs::symlink_metadata(&root).ok()?;
            let file = std::fs::symlink_metadata(root.join("events.jsonl")).ok()?;
            (metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && file.is_file()
                && !file.file_type().is_symlink())
            .then_some(root)
        })
        .collect()
}

#[derive(Clone, Serialize)]
pub struct TargetDto {
    id: String,
    name: String,
    config_path: String,
    output_path: String,
    status: String,
    reason: String,
    configurable: bool,
    kind: String,
    docs_url: String,
    verification: String,
    verified_records: usize,
}

#[derive(Serialize)]
pub struct PreviewDto {
    token: String,
    target: TargetDto,
    keys: Vec<String>,
    changes: Vec<(String, Value)>,
    receiver: bool,
    sync_config_path: Option<String>,
    sync_changes: Vec<(String, Value)>,
}

#[derive(Clone)]
struct Target {
    dto: TargetDto,
    path: PathBuf,
    changes: Vec<Change>,
    format: &'static str,
    receiver: bool,
    blocked: Option<&'static str>,
    sync_path: Option<PathBuf>,
    sync_changes: Vec<Change>,
    auth_app: PathBuf,
}

struct Context {
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    app: PathBuf,
    env: std::collections::HashMap<String, String>,
    port: u16,
    command_dirs: Vec<PathBuf>,
    external_manifests: Vec<(String, PathBuf)>,
    policy_files: Vec<(String, PathBuf)>,
}

impl Context {
    fn current(app: PathBuf, port: u16) -> Result<Self, String> {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or("no_home")?;
        let config = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .ok_or("no_home")?
        } else if cfg!(target_os = "macos") {
            home.join("Library/Application Support")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
        };
        let data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        // Only keys used for routing/policy checks; never return their values to the UI.
        let env = std::env::vars_os()
            .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
            .filter(|(k, _)| {
                k.starts_with("COPILOT_OTEL_")
                    || k.starts_with("OTEL_")
                    || k.starts_with("GEMINI_TELEMETRY_")
                    || k.starts_with("QWEN_TELEMETRY_")
                    || matches!(
                        k.as_str(),
                        "CODEX_HOME"
                            | "CLAUDE_CONFIG_DIR"
                            | "QWEN_HOME"
                            | "GEMINI_CLI_HOME"
                            | "COPILOT_HOME"
                            | "CODEBUDDY_CONFIG_DIR"
                            | "DISABLE_TELEMETRY"
                            | "BETA_TRACING_ENDPOINT"
                            | "CLAUDE_CODE_ENABLE_TELEMETRY"
                            | "CODEBUDDY_CODE_ENABLE_TELEMETRY"
                    )
            })
            .collect();
        Ok(Self {
            home,
            config,
            data,
            app,
            env,
            port,
            command_dirs: std::env::var_os("PATH")
                .map(|p| {
                    std::env::split_paths(&p)
                        .filter(|p| p.is_absolute())
                        .take(128)
                        .collect()
                })
                .unwrap_or_default(),
            external_manifests: external_manifests(),
            policy_files: system_policies(),
        })
    }
    fn agent_dir(&self, variable: &str, default: &str) -> PathBuf {
        self.env
            .get(variable)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(default))
    }
    fn output(&self, id: &str) -> PathBuf {
        self.app.join("telemetry").join(id).join("events.jsonl")
    }
    fn has_command(&self, command: &str) -> bool {
        let suffixes: &[&str] = if cfg!(windows) {
            &[".exe", ".cmd", ".ps1", ""]
        } else {
            &[""]
        };
        self.command_dirs.iter().any(|dir| {
            suffixes
                .iter()
                .any(|suffix| dir.join(format!("{command}{suffix}")).is_file())
        })
    }
}

fn external_manifests() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for (channel, install) in [
        ("Code", "Microsoft VS Code"),
        ("Code - Insiders", "Microsoft VS Code Insiders"),
    ] {
        if cfg!(windows) {
            for key in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Some(dir) = std::env::var_os(key) {
                    let root = PathBuf::from(dir).join(install);
                    out.push((
                        channel.into(),
                        root.join("resources/app/extensions/copilot/package.json"),
                    ));
                    if let Ok(entries) = std::fs::read_dir(root) {
                        for entry in entries.flatten().take(64) {
                            out.push((
                                channel.into(),
                                entry
                                    .path()
                                    .join("resources/app/extensions/copilot/package.json"),
                            ));
                        }
                    }
                }
            }
        } else if cfg!(target_os = "macos") {
            let app = if channel == "Code" {
                "Visual Studio Code.app"
            } else {
                "Visual Studio Code - Insiders.app"
            };
            out.push((
                channel.into(),
                PathBuf::from("/Applications")
                    .join(app)
                    .join("Contents/Resources/app/extensions/copilot/package.json"),
            ));
        } else {
            let root = if channel == "Code" {
                "/usr/share/code"
            } else {
                "/usr/share/code-insiders"
            };
            out.push((
                channel.into(),
                PathBuf::from(root).join("resources/app/extensions/copilot/package.json"),
            ));
        }
    }
    out
}

fn system_policies() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for (id, win, mac, linux, var) in [
        (
            "claude",
            "C:/Program Files/ClaudeCode/managed-settings.json",
            "/Library/Application Support/ClaudeCode/managed-settings.json",
            "/etc/claude-code/managed-settings.json",
            "",
        ),
        (
            "gemini",
            "C:/ProgramData/gemini-cli/settings.json",
            "/Library/Application Support/GeminiCli/settings.json",
            "/etc/gemini-cli/settings.json",
            "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        ),
        (
            "qwen",
            "C:/ProgramData/qwen-code/settings.json",
            "/Library/Application Support/QwenCode/settings.json",
            "/etc/qwen-code/settings.json",
            "QWEN_CODE_SYSTEM_SETTINGS_PATH",
        ),
    ] {
        let path = std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(if cfg!(windows) {
                win
            } else if cfg!(target_os = "macos") {
                mac
            } else {
                linux
            })
        });
        out.push((id.into(), path));
    }
    out
}

fn change(path: &[&str], value: Value) -> Change {
    (path.iter().map(|s| s.to_string()).collect(), value)
}
fn target(
    ctx: &Context,
    id: &str,
    name: &str,
    path: PathBuf,
    (format, receiver): (&'static str, bool),
    docs: &str,
    changes: Vec<Change>,
) -> Target {
    Target {
        dto: TargetDto {
            id: id.into(),
            name: name.into(),
            config_path: path.display().to_string(),
            output_path: if receiver {
                ctx.app
                    .join("telemetry/otlp-logs.jsonl")
                    .display()
                    .to_string()
            } else {
                ctx.output(id).display().to_string()
            },
            status: "missing".into(),
            reason: String::new(),
            configurable: true,
            kind: format.into(),
            docs_url: docs.into(),
            verification: "waiting".into(),
            verified_records: 0,
        },
        path,
        changes,
        format,
        receiver,
        blocked: None,
        sync_path: None,
        sync_changes: vec![],
        auth_app: ctx.app.clone(),
    }
}

fn readable(path: &Path) -> Result<Option<Vec<u8>>, String> {
    safe_path(path)?;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("unreadable_config".into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreadable_config")?;
    if bytes.len() as u64 > MAX_CONFIG {
        return Err("oversized_config".into());
    }
    Ok(Some(bytes))
}

/// Reject links/reparse points and relative paths rather than writing outside the shown target.
fn safe_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("unsafe_path".into());
    }
    for part in path.ancestors() {
        match std::fs::symlink_metadata(part) {
            Ok(meta) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err("unsafe_path".into());
                    }
                }
                if meta.file_type().is_symlink() {
                    return Err("unsafe_path".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("unreadable_config".into()),
        }
    }
    Ok(())
}

fn parse(target: &Target, bytes: Option<&[u8]>) -> Result<Value, String> {
    let text =
        std::str::from_utf8(bytes.unwrap_or(if target.format == "toml" { b"" } else { b"{}" }))
            .map_err(|_| "invalid_config")?;
    if target.format == "toml" {
        edit::toml_value(text)
    } else {
        edit::json_value(text)
    }
}

fn installed_copilot_keys(ctx: &Context, channel: &str) -> Option<Vec<String>> {
    let extension_root = ctx.home.join(if channel == "Code" {
        ".vscode/extensions"
    } else {
        ".vscode-insiders/extensions"
    });
    let mut manifests = Vec::new();
    let editor_command = if channel == "Code" {
        "code"
    } else {
        "code-insiders"
    };
    // Extension and user directories survive uninstalling the IDE. Require its launcher
    // for standalone extensions; a bundled package is itself an installation manifest.
    if ctx.has_command(editor_command) {
        if let Ok(entries) = std::fs::read_dir(extension_root) {
            for entry in entries.flatten().take(2048) {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("github.copilot-chat-")
                {
                    manifests.push(entry.path().join("package.json"));
                }
            }
        }
    }
    // Newer VS Code packages Copilot as a built-in extension.
    {
        let install = if channel == "Code" {
            "Microsoft VS Code"
        } else {
            "Microsoft VS Code Insiders"
        };
        let install_root = ctx.data.join("Programs").join(install);
        manifests.push(install_root.join("resources/app/extensions/copilot/package.json"));
        // Windows updater installs versioned directories (e.g. 07f806f999/resources/app).
        if let Ok(entries) = std::fs::read_dir(&install_root) {
            for entry in entries.flatten().take(64) {
                manifests.push(
                    entry
                        .path()
                        .join("resources/app/extensions/copilot/package.json"),
                );
            }
        }
    }
    manifests.extend(
        ctx.external_manifests
            .iter()
            .filter(|(c, _)| c == channel)
            .map(|(_, p)| p.clone()),
    );
    let mut keys = std::collections::BTreeSet::new();
    let mut installed = false;
    for path in manifests {
        let Ok(Some(bytes)) = readable(&path) else {
            continue;
        };
        let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let name = doc["name"].as_str().unwrap_or_default();
        if !matches!(name, "copilot-chat" | "copilot")
            || !doc["publisher"]
                .as_str()
                .is_some_and(|p| p.eq_ignore_ascii_case("GitHub"))
        {
            continue;
        }
        installed = true;
        // The built-in package's sibling app manifest establishes the independently
        // verified Agent Host baseline. Other versions remain manual until verified.
        if path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(|app| readable(&app.join("package.json")).ok().flatten())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| v["version"] == "1.140.0")
        {
            for suffix in ["enabled", "exporterType", "outfile", "captureContent"] {
                keys.insert(format!("chat.agentHost.otel.{suffix}"));
            }
        }
        let Some(config) = doc.pointer("/contributes/configuration") else {
            continue;
        };
        let sections = if let Some(a) = config.as_array() {
            a.clone()
        } else {
            vec![config.clone()]
        };
        for section in sections {
            if let Some(p) = section["properties"].as_object() {
                keys.extend(
                    p.keys()
                        .filter(|k| k.starts_with("github.copilot.chat.otel."))
                        .cloned(),
                );
            }
        }
    }
    installed.then(|| keys.into_iter().collect())
}

fn codebuddy_auth_version(ctx: &Context) -> bool {
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".ps1", ""]
    } else {
        &[""]
    };
    let Some(dir) = ctx.command_dirs.iter().find(|dir| {
        suffixes
            .iter()
            .any(|suffix| dir.join(format!("codebuddy{suffix}")).is_file())
    }) else {
        return false;
    };
    let package = dir.join("node_modules/@tencent-ai/codebuddy-code/package.json");
    readable(&package)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|manifest| {
            manifest["name"] == "@tencent-ai/codebuddy-code" && manifest["version"] == "2.98.0"
        })
}

fn discover(ctx: &Context) -> Vec<Target> {
    let mut targets = Vec::new();
    for (channel, id) in [
        ("Code", "copilot-vscode"),
        ("Code - Insiders", "copilot-vscode-insiders"),
    ] {
        let user = ctx.config.join(channel).join("User");
        let Some(keys) = installed_copilot_keys(ctx, channel) else {
            continue;
        };
        let mut profiles = vec![(
            id.to_owned(),
            format!("Copilot · {channel}"),
            user.join("settings.json"),
        )];
        if let Ok(entries) = std::fs::read_dir(user.join("profiles")) {
            for entry in entries.flatten().take(64) {
                if entry.path().join("settings.json").is_file() {
                    let profile = entry.file_name().to_string_lossy().into_owned();
                    profiles.push((
                        format!("{id}-profile-{profile}"),
                        format!("Copilot · {channel} · {profile}"),
                        entry.path().join("settings.json"),
                    ));
                }
            }
        }
        for (id, name, path) in profiles {
            let prefix = "github.copilot.chat.otel.";
            let mut changes = vec![
                change(&[&format!("{prefix}enabled")], json!(true)),
                change(&[&format!("{prefix}exporterType")], json!("file")),
                change(&[&format!("{prefix}outfile")], json!(ctx.output(&id))),
                change(&[&format!("{prefix}captureContent")], json!(false)),
            ];
            let supported = changes.iter().all(|(path, _)| keys.contains(&path[0]));
            if keys.contains(&format!("{prefix}captureIdentity")) {
                changes.push(change(&[&format!("{prefix}captureIdentity")], json!(false)));
            }
            // Local absolute paths must not propagate via Settings Sync.
            changes.push(change(&["settingsSync.ignoredSettings"], json!([])));
            let mut row = target(
                ctx,
                &id,
                &name,
                path.clone(),
                ("jsonc", false),
                COPILOT_DOC,
                changes,
            );
            if id.contains("-profile-") {
                row.sync_path = Some(user.join("settings.json"));
            }
            if !supported {
                row.blocked = Some("unsupported_version");
            }
            if ctx
                .env
                .keys()
                .any(|k| k.starts_with("COPILOT_OTEL_") || k.starts_with("OTEL_"))
            {
                row.blocked = Some("environment_override");
            }
            targets.push(row);
            if keys.contains(&"chat.agentHost.otel.enabled".to_string()) {
                let host_id = format!("{id}-agent-host");
                let mut changes = vec![
                    change(&["chat.agentHost.otel.enabled"], json!(true)),
                    change(&["chat.agentHost.otel.exporterType"], json!("file")),
                    change(
                        &["chat.agentHost.otel.outfile"],
                        json!(ctx.output(&host_id)),
                    ),
                    change(&["chat.agentHost.otel.captureContent"], json!(false)),
                    change(&["settingsSync.ignoredSettings"], json!([])),
                ];
                changes.shrink_to_fit();
                let mut host = target(
                    ctx,
                    &host_id,
                    &format!("{name} · Agent Host"),
                    path,
                    ("jsonc", false),
                    COPILOT_DOC,
                    changes,
                );
                if id.contains("-profile-") {
                    host.sync_path = Some(user.join("settings.json"));
                }
                if ctx
                    .env
                    .keys()
                    .any(|k| k.starts_with("COPILOT_OTEL_") || k.starts_with("OTEL_"))
                {
                    host.blocked = Some("environment_override");
                }
                targets.push(host);
            }
        }
    }
    for (id,name,var,dir,docs) in [
        ("gemini","Gemini CLI","GEMINI_CLI_HOME",".gemini","https://geminicli.com/docs/cli/telemetry/"),
        ("qwen","Qwen Code","QWEN_HOME",".qwen","https://github.com/QwenLM/qwen-code/blob/main/docs/developers/development/telemetry.md")
    ] {
        let dir = if id=="gemini" && ctx.env.get(var).is_some_and(|v|!v.is_empty()) {
            ctx.agent_dir(var,dir).join(".gemini")
        } else if id=="qwen" {
            match ctx.env.get(var).filter(|v|!v.is_empty()) {
                Some(value) if value=="~"=>ctx.home.clone(),
                Some(value) if value.starts_with("~/") || value.starts_with("~\\")=>ctx.home.join(&value[2..]),
                _=>ctx.agent_dir(var,dir)
            }
        } else {ctx.agent_dir(var,dir)};
        if !ctx.has_command(id) {continue;}
        let mut changes=vec![change(&["telemetry","enabled"],json!(true)),change(&["telemetry","outfile"],json!(ctx.output(id))),change(&["telemetry","logPrompts"],json!(false))];
        if id=="gemini" {changes.push(change(&["telemetry","target"],json!("local")));}
        else {changes.push(change(&["telemetry","includeSensitiveSpanAttributes"],json!(false)));}
        let mut row=target(ctx,id,name,dir.join("settings.json"),("jsonc",false),docs,changes);
        let prefix=if id=="gemini" {"GEMINI_TELEMETRY_"} else {"QWEN_TELEMETRY_"};
        if ctx.env.keys().any(|k| k.starts_with(prefix)) {row.blocked=Some("environment_override");}
        targets.push(row);
    }
    let claude = ctx.agent_dir("CLAUDE_CONFIG_DIR", ".claude");
    if ctx.has_command("claude") {
        let envs = [
            ("CLAUDE_CODE_ENABLE_TELEMETRY", "1"),
            ("OTEL_LOGS_EXPORTER", "otlp"),
            ("OTEL_EXPORTER_OTLP_LOGS_PROTOCOL", "http/json"),
            ("OTEL_LOG_USER_PROMPTS", "0"),
            ("OTEL_LOG_ASSISTANT_RESPONSES", "0"),
        ];
        let mut changes = envs
            .iter()
            .map(|(k, v)| change(&["env", k], json!(v)))
            .collect::<Vec<_>>();
        changes.push(change(
            &["env", "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"],
            json!(format!("http://127.0.0.1:{}/v1/logs", ctx.port)),
        ));
        let mut row = target(
            ctx,
            "claude",
            "Claude Code",
            claude.join("settings.json"),
            ("jsonc", true),
            "https://code.claude.com/docs/en/monitoring-usage",
            changes,
        );
        if claude.join("managed-settings.json").exists() {
            row.blocked = Some("managed_policy");
        }
        if ctx.env.keys().any(|k| {
            k.starts_with("OTEL_")
                || matches!(
                    k.as_str(),
                    "CLAUDE_CODE_ENABLE_TELEMETRY" | "BETA_TRACING_ENDPOINT"
                )
        }) {
            row.blocked = Some("environment_override");
        }
        targets.push(row);
    }
    let codex = ctx.agent_dir("CODEX_HOME", ".codex");
    if ctx.has_command("codex") {
        let changes = vec![
            change(&["otel", "log_user_prompt"], json!(false)),
            change(
                &["otel", "exporter"],
                json!({"otlp-http":{"endpoint":format!("http://127.0.0.1:{}/v1/logs",ctx.port),"protocol":"json"}}),
            ),
        ];
        let mut row = target(
            ctx,
            "codex",
            "Codex",
            codex.join("config.toml"),
            ("toml", true),
            "https://developers.openai.com/codex/config-sample/",
            changes,
        );
        if codex.join("requirements.toml").exists() || codex.join("managed_config.toml").exists() {
            row.blocked = Some("managed_policy");
        }
        targets.push(row);
    }
    if ctx.has_command("copilot") {
        let path = ctx
            .app
            .join("telemetry/copilot-cli")
            .join(if cfg!(windows) {
                "copilot-otel.ps1"
            } else {
                "copilot-otel.sh"
            });
        let mut row=target(ctx,"copilot-cli","Copilot CLI",path,("launcher",false),"https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference#opentelemetry-monitoring",vec![]);
        if ctx
            .env
            .keys()
            .any(|k| k.starts_with("COPILOT_OTEL_") || k.starts_with("OTEL_"))
        {
            row.blocked = Some("environment_override");
        }
        targets.push(row);
    }
    let codebuddy = ctx.agent_dir("CODEBUDDY_CONFIG_DIR", ".codebuddy");
    if ctx.has_command("codebuddy") {
        // CodeBuddy only exports traces. Keep supplemental traces out of auto-scanned roots.
        let envs = [
            ("CODEBUDDY_CODE_ENABLE_TELEMETRY", "1"),
            ("OTEL_TRACES_EXPORTER", "otlp"),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"),
            ("OTEL_LOG_USER_PROMPTS", "0"),
            ("OTEL_LOG_TOOL_DETAILS", "0"),
            ("OTEL_LOG_TOOL_CONTENT", "0"),
        ];
        let mut changes = envs
            .iter()
            .map(|(k, v)| change(&["env", k], json!(v)))
            .collect::<Vec<_>>();
        changes.push(change(
            &["env", "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"],
            json!(format!(
                "http://127.0.0.1:{}/v1/traces/supplemental",
                ctx.port
            )),
        ));
        let mut row = target(
            ctx,
            "codebuddy",
            "CodeBuddy",
            codebuddy.join("settings.json"),
            ("jsonc", true),
            "https://www.codebuddy.ai/docs/zh/cli/monitoring",
            changes,
        );
        row.dto.output_path = ctx
            .app
            .join("telemetry/otlp-traces.jsonl")
            .display()
            .to_string();
        // Exact documented release; neither an unknown binary nor a newer manifest
        // inherits this evidence. Do not execute the CLI to manufacture verification.
        if !codebuddy_auth_version(ctx) {
            row.blocked = Some("unsupported_version");
        }
        if codebuddy.join("managed-settings.json").exists() {
            row.blocked = Some("managed_policy");
        }
        if ctx.env.keys().any(|k| {
            k.starts_with("OTEL_")
                || matches!(
                    k.as_str(),
                    "CODEBUDDY_CODE_ENABLE_TELEMETRY" | "DISABLE_TELEMETRY"
                )
        }) {
            row.blocked = Some("environment_override");
        }
        targets.push(row);
    }
    for row in &mut targets {
        if ctx
            .policy_files
            .iter()
            .any(|(id, p)| id == &row.dto.id && p.exists())
        {
            row.blocked = Some("managed_policy");
        }
    }
    targets
}

fn inspect(mut target: Target) -> Target {
    if let Some(reason) = target.blocked {
        target.dto.status = "blocked".into();
        target.dto.reason = reason.into();
        target.dto.configurable = false;
        return target;
    }
    let result = (|| {
        let bytes = readable(&target.path)?;
        if target.format == "launcher" {
            if bytes.is_some() {
                if bytes.as_deref() != Some(launcher(Path::new(&target.dto.output_path)).as_bytes())
                {
                    return Err("config_changed".into());
                }
                target.dto.status = "configured".into();
                target.dto.configurable = false;
            }
            return Ok::<_, String>(());
        }
        let value = parse(&target, bytes.as_deref())?;
        let mut configured = if target.dto.id.starts_with("copilot-vscode") {
            if value["telemetry.telemetryLevel"] == "off" {
                return Err("telemetry_disabled".into());
            }
            let prefix = if target.dto.id.ends_with("-agent-host") {
                "chat.agentHost.otel."
            } else {
                "github.copilot.chat.otel."
            };
            let has_file = value[format!("{prefix}outfile")]
                .as_str()
                .is_some_and(|v| !v.trim().is_empty());
            let has_endpoint = value[format!("{prefix}otlpEndpoint")]
                .as_str()
                .is_some_and(|v| !v.trim().is_empty());
            if has_file
                && value[format!("{prefix}exporterType")]
                    .as_str()
                    .is_none_or(|s| s == "file")
            {
                target.dto.output_path = value[format!("{prefix}outfile")].as_str().unwrap().into();
            } else if has_endpoint {
                let endpoint = value[format!("{prefix}otlpEndpoint")].as_str().unwrap();
                target.dto.verification = "external".into();
                if endpoint.starts_with("http://127.0.0.1:")
                    || endpoint.starts_with("http://localhost:")
                {
                    // Only an app-owned endpoint may be associated automatically.
                    let desired = target
                        .changes
                        .iter()
                        .find(|(p, _)| p == &[format!("{prefix}otlpEndpoint")])
                        .and_then(|(_, v)| v.as_str());
                    if desired == Some(endpoint) {
                        target.dto.verification = "waiting".into();
                    }
                }
            }
            if value[format!("{prefix}enabled")] != true && (has_file || has_endpoint) {
                return Err("existing_destination".into());
            }
            value[format!("{prefix}enabled")] == true && (has_file || has_endpoint)
        } else if target.dto.id == "codex" {
            let exporter = value.pointer("/otel/exporter");
            if let Some(exporter) = exporter {
                let actual = exporter["otlp-http"]["endpoint"].as_str();
                let desired = target
                    .changes
                    .iter()
                    .find(|(p, _)| p == &["otel", "exporter"])
                    .and_then(|(_, v)| v["otlp-http"]["endpoint"].as_str());
                if actual.is_some_and(|endpoint| Some(endpoint) != desired)
                    || exporter.get("otlp-grpc").is_some()
                {
                    target.dto.verification = "external".into();
                }
            }
            if exporter.is_some_and(|v| {
                v.as_str().is_some_and(|s| s != "none") || (!v.is_object() && !v.is_string())
            }) {
                return Err("invalid_config".into());
            }
            if let Some(map) = exporter.and_then(Value::as_object) {
                if map.len() > 1
                    || map
                        .keys()
                        .any(|k| !matches!(k.as_str(), "otlp-http" | "otlp-grpc"))
                    || map.values().any(|v| !v.is_object())
                {
                    return Err("invalid_config".into());
                }
                if map.contains_key("otlp-grpc")
                    && !map["otlp-grpc"]["endpoint"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                {
                    return Err("existing_destination".into());
                }
                // An existing HTTP exporter can contain headers/timeouts or other options.
                // Edit leaf values rather than replacing that table (including inline tables).
                let desired = target
                    .changes
                    .iter()
                    .find(|(p, _)| p == &["otel", "exporter"])
                    .map(|(_, v)| v["otlp-http"].clone())
                    .ok_or("invalid_config")?;
                target.changes.retain(|(p, _)| p != &["otel", "exporter"]);
                for key in ["endpoint", "protocol"] {
                    target.changes.push(change(
                        &["otel", "exporter", "otlp-http", key],
                        desired[key].clone(),
                    ));
                }
            }
            exporter.is_some_and(|v| {
                ["otlp-http", "otlp-grpc"]
                    .iter()
                    .any(|key| v[key]["endpoint"].as_str().is_some_and(|s| !s.is_empty()))
            })
        } else if target.dto.id == "claude" || target.dto.id == "codebuddy" {
            let codebuddy = target.dto.id == "codebuddy";
            let enabled = if codebuddy {
                "CODEBUDDY_CODE_ENABLE_TELEMETRY"
            } else {
                "CLAUDE_CODE_ENABLE_TELEMETRY"
            };
            let exporter = if codebuddy {
                "OTEL_TRACES_EXPORTER"
            } else {
                "OTEL_LOGS_EXPORTER"
            };
            let endpoint = if codebuddy {
                "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"
            } else {
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"
            };
            if value["env"]["DISABLE_TELEMETRY"] == "1" {
                return Err("telemetry_disabled".into());
            }
            let destination = [endpoint, "OTEL_EXPORTER_OTLP_ENDPOINT"]
                .iter()
                .any(|k| value["env"][k].as_str().is_some_and(|s| !s.is_empty()));
            if let Some(actual) = [endpoint, "OTEL_EXPORTER_OTLP_ENDPOINT"]
                .iter()
                .find_map(|k| value["env"][k].as_str().filter(|s| !s.is_empty()))
            {
                let desired = target
                    .changes
                    .iter()
                    .find(|(p, _)| p == &["env", endpoint])
                    .and_then(|(_, v)| v.as_str());
                if Some(actual) != desired {
                    target.dto.verification = "external".into();
                }
            }
            if destination && !matches!(value["env"][enabled].as_str(), Some("1" | "true")) {
                return Err("existing_destination".into());
            }
            matches!(value["env"][enabled].as_str(), Some("1" | "true"))
                && value["env"][exporter]
                    .as_str()
                    .is_some_and(|s| s.split(',').any(|p| p.trim() == "otlp"))
                && [endpoint, "OTEL_EXPORTER_OTLP_ENDPOINT"]
                    .iter()
                    .any(|k| value["env"][k].as_str().is_some_and(|s| !s.is_empty()))
        } else {
            if value["telemetry"]["outfile"]
                .as_str()
                .is_none_or(|s| s.is_empty())
                && value["telemetry"]["otlpEndpoint"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            {
                target.dto.verification = "external".into();
            }
            let destination = ["outfile", "otlpEndpoint"].iter().any(|k| {
                value["telemetry"][k]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            });
            if let Some(path) = value["telemetry"]["outfile"]
                .as_str()
                .filter(|p| !p.trim().is_empty())
            {
                target.dto.output_path = path.into();
            }
            if destination && value.pointer("/telemetry/enabled") != Some(&json!(true)) {
                return Err("existing_destination".into());
            }
            value.pointer("/telemetry/enabled") == Some(&json!(true))
                && ["outfile", "otlpEndpoint"].iter().any(|k| {
                    value["telemetry"][k]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                })
        };
        if target.receiver && crate::receiver_auth::Family::from_id(&target.dto.id).is_some() {
            let family = crate::receiver_auth::Family::from_id(&target.dto.id).unwrap();
            let codebuddy = target.dto.id == "codebuddy";
            let local_endpoint = format!(
                "http://127.0.0.1:{}{}",
                target.auth_port(),
                if codebuddy {
                    "/v1/traces/supplemental"
                } else {
                    "/v1/logs"
                }
            );
            if codebuddy
                && value
                    .pointer("/env/OTEL_EXPORTER_OTLP_TRACES_HEADERS")
                    .is_some()
            {
                // The 2.98.0 release proves generic headers. Signal-specific headers
                // in the rolling reference cannot certify this older installation.
                return Err("authentication_unverified".into());
            }
            let (actual_endpoint, header) = if target.dto.id == "codex" {
                (
                    value
                        .pointer("/otel/exporter/otlp-http/endpoint")
                        .and_then(Value::as_str),
                    value
                        .pointer("/otel/exporter/otlp-http/headers/Authorization")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
            } else {
                (
                    value
                        .pointer(if codebuddy {
                            "/env/OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"
                        } else {
                            "/env/OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"
                        })
                        .and_then(Value::as_str),
                    exporter_authorization(
                        &value,
                        if codebuddy {
                            "OTEL_EXPORTER_OTLP_HEADERS"
                        } else {
                            "OTEL_EXPORTER_OTLP_LOGS_HEADERS"
                        },
                    )?,
                )
            };
            if !configured && header.is_some() && actual_endpoint != Some(local_endpoint.as_str()) {
                return Err("existing_destination".into());
            }
            // Preserve existing destinations. Only our exact endpoint can offer credential repair.
            if configured && actual_endpoint == Some(local_endpoint.as_str()) {
                configured = header.as_deref().is_some_and(|h| {
                    crate::receiver_auth::configured(&target.auth_app, &target.path, family, h)
                });
            }
            if !configured {
                add_authentication(&mut target, &value, crate::receiver_auth::PLACEHOLDER)?;
            }
        }
        if configured {
            target.dto.status = "configured".into();
            target.dto.configurable = false;
        }
        // Preserve every user entry; explicit sync inclusions require manual review.
        if target.dto.id.starts_with("copilot-vscode") {
            // APPLICATION scope: Settings Sync exclusions always live in the default
            // user's settings, even when Copilot's file settings live in a profile.
            let sync_bytes = target
                .sync_path
                .as_ref()
                .map(|p| readable(p))
                .transpose()?
                .flatten();
            let sync_text = std::str::from_utf8(sync_bytes.as_deref().unwrap_or(b"{}"))
                .map_err(|_| "invalid_config")?;
            let sync_value = if target.sync_path.is_some() {
                edit::json_value(sync_text)?
            } else {
                value.clone()
            };
            let original = sync_value.get("settingsSync.ignoredSettings");
            if original.is_some_and(|v| !v.is_array()) {
                return Err("invalid_config".into());
            }
            let mut ignored = original
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let keys = target
                .changes
                .iter()
                .filter_map(|(p, _)| p.first())
                .filter(|k| {
                    k.starts_with("github.copilot.chat.otel.")
                        || k.starts_with("chat.agentHost.otel.")
                })
                .cloned()
                .collect::<Vec<_>>();
            if ignored.iter().any(|v| v.as_str().is_none()) {
                return Err("invalid_config".into());
            }
            for key in keys {
                if ignored.contains(&json!(format!("-{key}"))) {
                    return Err("sync_conflict".into());
                }
                if !ignored.contains(&json!(key)) {
                    ignored.push(json!(key));
                }
            }
            if target.sync_path.is_some() {
                target
                    .changes
                    .retain(|(p, _)| p != &["settingsSync.ignoredSettings"]);
                target.sync_changes =
                    vec![change(&["settingsSync.ignoredSettings"], json!(ignored))];
                edit::merge_json(sync_text, &target.sync_changes)?;
            } else if let Some((_, v)) = target
                .changes
                .iter_mut()
                .find(|(p, _)| p == &["settingsSync.ignoredSettings"])
            {
                *v = json!(ignored);
            }
        }
        // Validate the prospective merge before enabling a button (wrong nested types fail closed).
        if !configured {
            let text =
                std::str::from_utf8(bytes.as_deref().unwrap_or(if target.format == "toml" {
                    b""
                } else {
                    b"{}"
                }))
                .map_err(|_| "invalid_config")?;
            merge(&target, text)?;
        }
        Ok(())
    })();
    if let Err(reason) = result {
        target.dto.status = "blocked".into();
        target.dto.reason = reason;
        target.dto.configurable = false;
    }
    target
}

fn inspect_supported(target: Target) -> Target {
    let mut target = inspect(target);
    if target.receiver && !crate::receiver_auth::available() && target.dto.configurable {
        target.dto.status = "blocked".into();
        target.dto.reason = "credential_store_unavailable".into();
        target.dto.configurable = false;
    }
    target
}

impl Target {
    fn auth_port(&self) -> u16 {
        self.changes
            .iter()
            .find_map(|(p, v)| {
                let endpoint = if p == &["otel", "exporter"] {
                    v["otlp-http"]["endpoint"].as_str()
                } else if p.last().is_some_and(|p| {
                    p == "endpoint"
                        || p == "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"
                        || p == "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"
                }) {
                    v.as_str()
                } else {
                    None
                }?;
                endpoint
                    .strip_prefix("http://127.0.0.1:")?
                    .split('/')
                    .next()?
                    .parse()
                    .ok()
            })
            .unwrap_or(0)
    }
}
fn exporter_headers(value: &Value, key: &str) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value.get("env").and_then(|env| env.get(key)) else {
        return Ok(vec![]);
    };
    let text = value.as_str().ok_or("invalid_config")?;
    let mut entries = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for entry in text.split(',').filter(|s| !s.trim().is_empty()) {
        let (key, val) = entry.split_once('=').ok_or("invalid_config")?;
        let key = key.trim();
        if key.is_empty()
            || !names.insert(key.to_ascii_lowercase())
            || key.contains(['\r', '\n'])
            || val.contains(['\r', '\n'])
        {
            return Err("invalid_config".into());
        }
        entries.push((key.to_string(), val.trim().to_string()));
    }
    Ok(entries)
}
fn exporter_authorization(value: &Value, key: &str) -> Result<Option<String>, String> {
    Ok(exporter_headers(value, key)?
        .into_iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.replace("%20", " ")))
}
fn add_authentication(target: &mut Target, value: &Value, header: &str) -> Result<(), String> {
    if target.dto.id == "codex" {
        // When replacing 'none', insert the header in the newly created table.
        if let Some((_, exporter)) = target
            .changes
            .iter_mut()
            .find(|(p, _)| p == &["otel", "exporter"])
        {
            exporter["otlp-http"]["headers"] = json!({"Authorization":header});
        } else {
            let headers = value.pointer("/otel/exporter/otlp-http/headers");
            if headers.is_some_and(|h| !h.is_object()) {
                return Err("invalid_config".into());
            }
            if let Some(map) = headers.and_then(Value::as_object) {
                if map
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case("authorization") && k != "Authorization")
                {
                    return Err("invalid_config".into());
                }
            }
            let path = vec![
                "otel".into(),
                "exporter".into(),
                "otlp-http".into(),
                "headers".into(),
                "Authorization".into(),
            ];
            target.changes.retain(|(p, _)| p != &path);
            target.changes.push((path, json!(header)));
        }
    } else if matches!(target.dto.id.as_str(), "claude" | "codebuddy") {
        let codebuddy = target.dto.id == "codebuddy";
        let key = if codebuddy {
            "OTEL_EXPORTER_OTLP_HEADERS"
        } else {
            "OTEL_EXPORTER_OTLP_LOGS_HEADERS"
        };
        let mut entries = exporter_headers(value, key)?;
        entries.retain(|(k, _)| !k.eq_ignore_ascii_case("authorization"));
        entries.push((
            "Authorization".into(),
            if codebuddy {
                header.replace(' ', "%20")
            } else {
                header.into()
            },
        ));
        let path = vec!["env".into(), key.into()];
        target.changes.retain(|(p, _)| p != &path);
        target.changes.push((
            path,
            json!(entries
                .into_iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",")),
        ));
    } else {
        return Err("authentication_unverified".into());
    }
    Ok(())
}

fn merge(target: &Target, text: &str) -> Result<String, String> {
    if target.format == "toml" {
        edit::merge_toml(text, &target.changes)
    } else {
        edit::merge_json(text, &target.changes)
    }
}

fn launcher(path: &Path) -> String {
    let output = path.display().to_string();
    if cfg!(windows) {
        format!("# LLM Usage: run this launcher instead of copilot; environment restored on exit.\n$previous = @{{}}\n$values = @{{\n  COPILOT_OTEL_ENABLED = 'true'\n  COPILOT_OTEL_EXPORTER_TYPE = 'file'\n  COPILOT_OTEL_FILE_EXPORTER_PATH = '{}'\n  OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT = 'false'\n}}\ntry {{\n  foreach ($key in $values.Keys) {{ $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process'); [Environment]::SetEnvironmentVariable($key, $values[$key], 'Process') }}\n  & copilot @args\n}} finally {{\n  foreach ($key in $values.Keys) {{ [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }}\n}}\n", output.replace('\'',"''"))
    } else {
        format!("#!/bin/sh\n# LLM Usage: process-local configuration\nCOPILOT_OTEL_ENABLED=true COPILOT_OTEL_EXPORTER_TYPE=file COPILOT_OTEL_FILE_EXPORTER_PATH='{}' OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=false exec copilot \"$@\"\n",output.replace('\'',"'\"'\"'"))
    }
}

struct Plan {
    target: Target,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
    created: std::time::Instant,
    applied: bool,
    sync: Option<SyncEdit>,
    credential: Option<crate::receiver_auth::Binding>,
}

struct SyncEdit {
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
    changes: Vec<Change>,
}

fn prepare_sync(target: &Target) -> Result<Option<SyncEdit>, String> {
    let Some(path) = &target.sync_path else {
        return Ok(None);
    };
    let before = readable(path)?;
    let text =
        std::str::from_utf8(before.as_deref().unwrap_or(b"{}")).map_err(|_| "invalid_config")?;
    let after = edit::merge_json(text, &target.sync_changes)?.into_bytes();
    if before.as_deref() == Some(after.as_slice()) {
        return Ok(None);
    }
    Ok(Some(SyncEdit {
        path: path.clone(),
        before,
        after,
        changes: target.sync_changes.clone(),
    }))
}
static PLANS: OnceLock<Mutex<std::collections::HashMap<String, Plan>>> = OnceLock::new();
fn plans() -> &'static Mutex<std::collections::HashMap<String, Plan>> {
    PLANS.get_or_init(Default::default)
}
fn nonce() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        crate::scanner::now_ms(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn context(state: &AppState) -> Result<Context, String> {
    let app = state.db_path.parent().ok_or("no_home")?.to_owned();
    Context::current(
        app,
        state
            .settings
            .lock()
            .map_err(|_| "setup_busy")?
            .otel_receiver_port,
    )
}

fn receiver_setting(state: &AppState, enabled: bool) -> Result<bool, String> {
    let storage = state.storage.lock().map_err(|_| "setup_busy")?;
    let mut settings = state.settings.lock().map_err(|_| "setup_busy")?;
    let old = settings.otel_receiver_enabled;
    if old != enabled {
        let mut next = settings.clone();
        next.otel_receiver_enabled = enabled;
        save_settings(&storage, &next)?;
        *settings = next;
    }
    Ok(old)
}

#[tauri::command]
pub async fn telemetry_check(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<TargetDto>, String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&state)?;
        Ok(
            evidence::verify_all(discover(&ctx).into_iter().map(inspect_supported).collect())
                .into_iter()
                .map(|target| target.dto)
                .collect(),
        )
    })
    .await
    .map_err(|_| "setup_failed".to_string())?
}

fn preview_changes(target: &Target, value: &Value) -> Vec<(String, Value)> {
    target
        .changes
        .iter()
        .filter(|(p, v)| lookup(value, p) != Some(v))
        .map(|(p, v)| {
            (
                p.join("."),
                if target.receiver
                    && (p
                        .last()
                        .is_some_and(|k| k.contains("HEADERS") || k == "Authorization")
                        || p == &["otel", "exporter"])
                {
                    // Exporter tables/headers may retain user secrets; never serialize them into IPC.
                    if p == &["otel", "exporter"] {
                        let mut redacted = v.clone();
                        redacted["otlp-http"]["headers"] =
                            json!({"Authorization":crate::receiver_auth::PLACEHOLDER});
                        redacted
                    } else {
                        json!(crate::receiver_auth::PLACEHOLDER)
                    }
                } else {
                    v.clone()
                },
            )
        })
        .collect::<Vec<_>>()
}

#[tauri::command]
pub async fn telemetry_preview(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<PreviewDto, String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&state)?;
        let target = inspect_supported(
            discover(&ctx)
                .into_iter()
                .find(|t| t.dto.id == id)
                .ok_or("not_installed")?,
        );
        if !target.dto.configurable {
            return Err(if target.dto.reason.is_empty() {
                "already_configured".into()
            } else {
                target.dto.reason.clone()
            });
        }
        let before = readable(&target.path)?;
        let after = if target.format == "launcher" {
            launcher(&ctx.output(&target.dto.id)).into_bytes()
        } else {
            merge(
                &target,
                std::str::from_utf8(before.as_deref().unwrap_or(if target.format == "toml" {
                    b""
                } else {
                    b"{}"
                }))
                .map_err(|_| "invalid_config")?,
            )?
            .into_bytes()
        };
        let value = if target.format == "launcher" {
            json!({})
        } else {
            parse(&target, before.as_deref())?
        };
        let changes = preview_changes(&target, &value);
        let keys = changes.iter().map(|(p, _)| p.clone()).collect();
        let token = nonce();
        let sync = prepare_sync(&target)?;
        let result = PreviewDto {
            token: token.clone(),
            target: target.dto.clone(),
            keys,
            changes,
            receiver: target.receiver,
            sync_config_path: sync.as_ref().map(|s| s.path.display().to_string()),
            sync_changes: sync
                .as_ref()
                .map(|s| {
                    s.changes
                        .iter()
                        .map(|(p, v)| (p.join("."), v.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        let mut plans = plans().lock().map_err(|_| "setup_busy")?;
        plans.retain(|_, p| p.applied || p.created.elapsed().as_secs() < 600);
        if plans.len() >= 64 {
            return Err("setup_busy".into());
        }
        plans.insert(
            token,
            Plan {
                target,
                before,
                after,
                created: std::time::Instant::now(),
                applied: false,
                sync,
                credential: None,
            },
        );
        Ok(result)
    })
    .await
    .map_err(|_| "setup_failed".to_string())?
}

/// Writes through a same-directory temporary file. Backup required before replacing existing bytes.
fn commit(
    path: &Path,
    before: Option<&[u8]>,
    after: &[u8],
    backup_root: &Path,
    token: &str,
) -> Result<(), String> {
    safe_path(path)?;
    if readable(path)?.as_deref() != before {
        return Err("config_changed".into());
    }
    let parent = path.parent().ok_or("unsafe_path")?;
    std::fs::create_dir_all(parent).map_err(|_| "write_failed")?;
    let temp = parent.join(format!(".llm-usage-{token}.tmp"));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|_| "write_failed")?;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.permissions().readonly() {
                return Err("read_only".into());
            }
            std::fs::set_permissions(&temp, meta.permissions()).map_err(|_| "write_failed")?;
        }
        file.write_all(after)
            .and_then(|_| file.sync_all())
            .map_err(|_| "write_failed")?;
        drop(file);
        if let Some(bytes) = before {
            safe_path(backup_root)?;
            std::fs::create_dir_all(backup_root).map_err(|_| "backup_failed")?;
            let mut opts = std::fs::OpenOptions::new();
            opts.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut backup = opts
                .open(backup_root.join(format!("{token}.backup")))
                .map_err(|_| "backup_failed")?;
            backup
                .write_all(bytes)
                .and_then(|_| backup.sync_all())
                .map_err(|_| "backup_failed")?;
        }
        safe_path(path)?;
        if readable(path)?.as_deref() != before {
            return Err("config_changed".into());
        }
        std::fs::rename(&temp, path).map_err(|_| "write_failed")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}

fn restore_exact(
    path: &Path,
    current: &[u8],
    original: Option<&[u8]>,
    backup_root: &Path,
    token: &str,
) -> Result<(), String> {
    if let Some(original) = original {
        commit(path, Some(current), original, backup_root, token)
    } else {
        safe_path(path)?;
        if readable(path)?.as_deref() != Some(current) {
            return Err("config_changed".into());
        }
        std::fs::remove_file(path).map_err(|_| "write_failed".into())
    }
}

fn commit_plan(plan: &Plan, backup_root: &Path, token: &str) -> Result<(), String> {
    if let Some(sync) = &plan.sync {
        commit(
            &sync.path,
            sync.before.as_deref(),
            &sync.after,
            backup_root,
            &format!("{token}-sync"),
        )?;
    }
    let result = commit(
        &plan.target.path,
        plan.before.as_deref(),
        &plan.after,
        backup_root,
        token,
    );
    if result.is_err() {
        if let Some(sync) = &plan.sync {
            // CAS rollback: never restore over a concurrently edited default settings file.
            restore_exact(
                &sync.path,
                &sync.after,
                sync.before.as_deref(),
                backup_root,
                &format!("{token}-rollback"),
            )?;
        }
    }
    result
}

fn authenticated_commit(
    plan: &mut Plan,
    app: &Path,
    backup_root: &Path,
    token: &str,
    store: &dyn crate::receiver_auth::Store,
) -> Result<(), String> {
    if !plan.target.receiver {
        return commit_plan(plan, backup_root, token);
    }
    let family = crate::receiver_auth::Family::from_id(&plan.target.dto.id)
        .ok_or("authentication_unverified")?;
    let credential = crate::receiver_auth::issue(store, app, &plan.target.path, family)?;
    let original_target = plan.target.clone();
    let original_after = plan.after.clone();
    let result = (|| {
        let before = parse(&plan.target, plan.before.as_deref())?;
        add_authentication(&mut plan.target, &before, &credential.header())?;
        plan.after = merge(
            &plan.target,
            std::str::from_utf8(plan.before.as_deref().unwrap_or(
                if plan.target.format == "toml" {
                    b""
                } else {
                    b"{}"
                },
            ))
            .map_err(|_| "invalid_config")?,
        )?
        .into_bytes();
        commit_plan(plan, backup_root, token)
    })();
    if let Err(error) = result {
        plan.target = original_target;
        plan.after = original_after;
        crate::receiver_auth::revoke(store, &credential)?;
        return Err(error);
    }
    plan.credential = Some(credential);
    Ok(())
}

#[tauri::command]
pub async fn telemetry_apply(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
) -> Result<(), String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&state)?;
        let mut plans = plans().lock().map_err(|_| "setup_busy")?;
        let plan = plans.get_mut(&token).ok_or("preview_expired")?;
        if plan.applied {
            return Ok(());
        }
        if plan.created.elapsed().as_secs() > 600 {
            return Err("preview_expired".into());
        }
        let current = inspect_supported(
            discover(&ctx)
                .into_iter()
                .find(|t| t.dto.id == plan.target.dto.id)
                .ok_or("not_installed")?,
        );
        if !current.dto.configurable {
            return Err("configuration_blocked".into());
        }
        if current.path != plan.target.path
            || current.changes != plan.target.changes
            || current.sync_path != plan.target.sync_path
            || current.sync_changes != plan.target.sync_changes
        {
            return Err("config_changed".into());
        }
        if readable(&plan.target.path)? != plan.before {
            return Err("config_changed".into());
        }
        if let Some(sync) = &plan.sync {
            if readable(&sync.path)? != sync.before {
                return Err("config_changed".into());
            }
        } else if prepare_sync(&current)?.is_some() {
            return Err("config_changed".into());
        }
        safe_path(&ctx.output(&plan.target.dto.id))?;
        std::fs::create_dir_all(
            ctx.output(&plan.target.dto.id)
                .parent()
                .ok_or("unsafe_path")?,
        )
        .map_err(|_| "write_failed")?;
        let mut receiver_started = false;
        let mut previous_receiver = false;
        if plan.target.receiver {
            // The listener must actually bind before a user's Agent is pointed at it.
            if ctx.port == 0 {
                return Err("invalid_config".into());
            }
            receiver_started =
                crate::otel_receiver::ensure_started_owned(ctx.port, ctx.app.join("otel"))?;
            match receiver_setting(&state, true) {
                Ok(old) => previous_receiver = old,
                Err(error) => {
                    if receiver_started {
                        crate::otel_receiver::stop_owned(ctx.port, &ctx.app.join("otel"));
                    }
                    return Err(error);
                }
            }
        }
        if let Err(error) = authenticated_commit(
            plan,
            &ctx.app,
            &ctx.app.join("telemetry-backups"),
            &token,
            &crate::receiver_auth::SystemStore,
        ) {
            if plan.target.receiver {
                let restored = receiver_setting(&state, previous_receiver);
                if receiver_started {
                    crate::otel_receiver::stop_owned(ctx.port, &ctx.app.join("otel"));
                }
                restored?;
            }
            return Err(error);
        }
        plan.applied = true;
        Ok(())
    })
    .await
    .map_err(|_| "setup_failed".to_string())?
}

#[tauri::command]
pub async fn telemetry_undo(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
) -> Result<Vec<String>, String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&state)?;
        let mut plans = plans().lock().map_err(|_| "setup_busy")?;
        let plan = plans.get_mut(&token).ok_or("preview_expired")?;
        if !plan.applied {
            return Err("preview_expired".into());
        }
        if let Some(credential) = &plan.credential {
            crate::receiver_auth::revoke(&crate::receiver_auth::SystemStore, credential)?;
        }
        let current = readable(&plan.target.path)?.ok_or("config_changed")?;
        let mut conflicts = if current == plan.after {
            if let Some(before) = &plan.before {
                commit(
                    &plan.target.path,
                    Some(&current),
                    before,
                    &ctx.app.join("telemetry-backups"),
                    &nonce(),
                )?;
            } else {
                safe_path(&plan.target.path)?;
                std::fs::remove_file(&plan.target.path).map_err(|_| "write_failed")?;
            }
            vec![]
        } else {
            if plan.target.format == "launcher" {
                return Err("config_changed".into());
            }
            let original = parse(&plan.target, plan.before.as_deref())?;
            let changes = plan
                .target
                .changes
                .iter()
                .filter(|(p, v)| lookup(&original, p) != Some(v))
                .cloned()
                .collect::<Vec<_>>();
            let text = std::str::from_utf8(&current).map_err(|_| "invalid_config")?;
            let (output, conflicts) = if plan.target.format == "toml" {
                edit::restore_toml(text, &original, &changes)?
            } else {
                edit::restore_json(text, &original, &changes)?
            };
            if output.as_bytes() != current {
                commit(
                    &plan.target.path,
                    Some(&current),
                    output.as_bytes(),
                    &ctx.app.join("telemetry-backups"),
                    &nonce(),
                )?;
            }
            conflicts
        };
        if let Some(sync) = &plan.sync {
            let current = readable(&sync.path)?.ok_or("config_changed")?;
            if current == sync.after {
                restore_exact(
                    &sync.path,
                    &current,
                    sync.before.as_deref(),
                    &ctx.app.join("telemetry-backups"),
                    &nonce(),
                )?;
            } else {
                let original = edit::json_value(
                    std::str::from_utf8(sync.before.as_deref().unwrap_or(b"{}"))
                        .map_err(|_| "invalid_config")?,
                )?;
                let (output, skipped) = edit::restore_json(
                    std::str::from_utf8(&current).map_err(|_| "invalid_config")?,
                    &original,
                    &sync.changes,
                )?;
                if output.as_bytes() != current {
                    commit(
                        &sync.path,
                        Some(&current),
                        output.as_bytes(),
                        &ctx.app.join("telemetry-backups"),
                        &nonce(),
                    )?;
                }
                conflicts.extend(skipped);
            }
        }
        plan.applied = false;
        Ok(conflicts)
    })
    .await
    .map_err(|_| "setup_failed".to_string())?
}

#[cfg(test)]
mod tests;
