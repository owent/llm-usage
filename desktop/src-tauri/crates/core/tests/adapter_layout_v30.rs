//! V30 结构检查：每个已实现 Agent 独立目录（mod/detect/versions），根级不再有
//! 该 Agent 的单文件实现；产品特有映射不留在根级 usage_map.rs。
//! 目录存在或编译通过不能代替版本兼容验收（兼容行为在各适配器 V17 测试中验证），
//! 本测试只锁定目录合同本身，防止后续新增 Agent 又回到根级单文件。

const AGENTS: &[&str] = &["codex", "claude", "pi", "omp", "gemini", "qwen"];

fn adapters_src() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("adapters")
}

#[test]
fn every_implemented_agent_lives_in_its_own_directory() {
    let root = adapters_src();
    for agent in AGENTS {
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
        // 根级不再有该 Agent 的单文件实现。
        assert!(
            !root.join(format!("{agent}.rs")).is_file(),
            "root-level {agent}.rs must not exist; implementations live in {agent}/"
        );
    }
}

#[test]
fn product_specific_mappings_left_root_usage_map() {
    // V30：产品特有映射不留在根级单文件。codex/claude 映射已下沉；
    // pi/omp（map_pi_family）与 gemini/qwen（map_genai_usage）是固定源码证实的
    // 跨 Agent 共享口径，允许留在根级（usage_map.rs 文件头有登记）。
    let usage_map = std::fs::read_to_string(adapters_src().join("usage_map.rs")).unwrap();
    assert!(
        !usage_map.contains("pub fn map_codex"),
        "codex product mapping must live in adapters/codex/"
    );
    assert!(
        !usage_map.contains("pub fn map_claude"),
        "claude product mapping must live in adapters/claude/"
    );
    // 允许保留的跨 Agent 共享映射确实存在（防止误删共享件）。
    assert!(usage_map.contains("pub fn map_pi_family"));
    assert!(usage_map.contains("pub fn map_genai_usage"));
}

#[test]
fn each_agent_registry_declares_verified_versions_and_latest() {
    // 注册表形状检查：每个 Agent 的 versions/mod.rs 登记已验证版本映射与
    // 最新实现常量（探测/扫描共用 select 的单一事实来源）。
    for agent in AGENTS {
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
