//! V30 structure: each implemented agent has mod/detect/versions in its own directory;
//! no root-level single-file implementation or product mapping remains in usage_map.rs.
//! Directories/compilation do not verify version compatibility; adapter V17 tests check that.
//! These tests enforce directory structure for future agents.

use llm_usage_core::adapters::built_in_adapters;
use std::collections::BTreeSet;

fn adapters_src() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("adapters")
}

fn registered_adapter_ids() -> Vec<&'static str> {
    let adapters = built_in_adapters();
    assert!(
        !adapters.is_empty(),
        "the built-in adapter registry must not be empty"
    );
    adapters
        .iter()
        .map(|adapter| adapter.adapter_id())
        .collect()
}

fn adapter_directory(adapter_id: &str) -> &str {
    match adapter_id {
        "copilot-chat" => "copilot_chat",
        "vs-copilot" => "vs_copilot",
        // Rust module aliases do not rename directories: continue, gajae-code,
        // kimi-code, kimi-work and mimo-code already use their literal IDs.
        other => other,
    }
}

#[test]
fn built_in_adapter_ids_are_unique() {
    let mut seen = BTreeSet::new();
    for adapter_id in registered_adapter_ids() {
        assert!(
            seen.insert(adapter_id),
            "duplicate built-in adapter ID: {adapter_id}"
        );
    }
}

#[test]
fn every_implemented_agent_lives_in_its_own_directory() {
    let root = adapters_src();
    for agent in registered_adapter_ids() {
        let directory = adapter_directory(agent);
        let dir = root.join(directory);
        assert!(
            dir.is_dir(),
            "registered adapter {agent} must have its own {directory}/ directory"
        );
        assert!(
            dir.join("mod.rs").is_file(),
            "{directory}/mod.rs (稳定入口) missing"
        );
        assert!(
            dir.join("detect.rs").is_file(),
            "{directory}/detect.rs (探测与版本分派) missing"
        );
        assert!(
            dir.join("versions").join("mod.rs").is_file(),
            "{directory}/versions/mod.rs (版本注册表) missing"
        );
        // No root-level single-file implementation for this agent.
        assert!(
            !root.join(format!("{directory}.rs")).is_file(),
            "root-level {directory}.rs must not exist; implementations live in {directory}/"
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
    for agent in registered_adapter_ids() {
        let directory = adapter_directory(agent);
        let registry = std::fs::read_to_string(
            adapters_src()
                .join(directory)
                .join("versions")
                .join("mod.rs"),
        )
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
