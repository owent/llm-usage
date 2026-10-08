//! V30 structure: each implemented agent has mod/detect/versions in its own directory;
//! no root-level single-file implementation or product mapping remains in usage_map.rs.
//! Directories/compilation do not verify version compatibility; adapter V17 tests check that.
//! These tests enforce directory structure for future agents.

// M8 second-batch adapters (documentation-level implementation, 2026-09-29) follow the same layout.
// gajae-code/continue directory names differ from module names through #[path]; check directories.
const M8_AGENTS: &[&str] = &[
    "zed",
    "aider",
    "junie",
    "xum",
    "droid",
    "amp",
    "grok",
    "roo",
    "goose",
    "crush",
    "jcode",
    "gajae-code",
    "commandcode",
    "continue",
    "atomcode",
    "kiro",
    "antigravity",
    "qoder",
];
const M4_M5_BUDDY_AGENTS: &[&str] = &["codebuddy", "workbuddy"];
const AGENTS: &[&str] = &["codex", "claude", "pi", "omp", "gemini", "qwen"];

fn adapters_src() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("adapters")
}

#[test]
fn every_implemented_agent_lives_in_its_own_directory() {
    let root = adapters_src();
    let all: Vec<&str> = AGENTS
        .iter()
        .chain(M8_AGENTS.iter())
        .chain(M4_M5_BUDDY_AGENTS.iter())
        .copied()
        .collect();
    for agent in all {
        let dir = root.join(agent);
        assert!(dir.is_dir(), "agent {agent} must have its own directory");
        assert!(
            dir.join("mod.rs").is_file(),
            "{agent}/mod.rs (稳定入口) missing"
        );
        assert!(
            dir.join("detect.rs").is_file(),
            "{agent}/detect.rs (探测与版本分派) missing"
        );
        assert!(
            dir.join("versions").join("mod.rs").is_file(),
            "{agent}/versions/mod.rs (版本注册表) missing"
        );
        // No root-level single-file implementation for this agent.
        assert!(
            !root.join(format!("{agent}.rs")).is_file(),
            "root-level {agent}.rs must not exist; implementations live in {agent}/"
        );
    }
}

#[test]
fn product_specific_mappings_left_root_usage_map() {
    // V30: Codex/Claude product mappings moved into their directories.
    // pi/omp map_pi_family and gemini/qwen map_genai_usage have source-verified
    // shared field semantics and may remain at root, as documented in usage_map.rs.
    let usage_map = std::fs::read_to_string(adapters_src().join("usage_map.rs")).unwrap();
    assert!(
        !usage_map.contains("pub fn map_codex"),
        "codex product mapping must live in adapters/codex/"
    );
    assert!(
        !usage_map.contains("pub fn map_claude"),
        "claude product mapping must live in adapters/claude/"
    );
    assert!(
        !usage_map.contains("pub fn map_copilot"),
        "copilot product mapping must live in adapters/copilot/ (M5 下沉)"
    );
    // Retain the permitted shared mappings.
    assert!(usage_map.contains("pub fn map_pi_family"));
    assert!(usage_map.contains("pub fn map_genai_usage"));
}

#[test]
fn each_agent_registry_declares_verified_versions_and_latest() {
    // Every versions/mod.rs declares verified version mappings and the latest
    // implementation constant; detection/scanning share select as their version policy.
    let all: Vec<&str> = AGENTS
        .iter()
        .chain(M8_AGENTS.iter())
        .chain(M4_M5_BUDDY_AGENTS.iter())
        .copied()
        .collect();
    for agent in all {
        let registry =
            std::fs::read_to_string(adapters_src().join(agent).join("versions").join("mod.rs"))
                .unwrap();
        assert!(
            registry.contains("VERIFIED_VERSION_IMPLS"),
            "{agent} registry must declare VERIFIED_VERSION_IMPLS"
        );
        assert!(
            registry.contains("LATEST_IMPL_ID"),
            "{agent} registry must declare LATEST_IMPL_ID"
        );
        assert!(
            registry.contains("fn select"),
            "{agent} registry must expose select() shared by detect and scan"
        );
    }
}
