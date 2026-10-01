//! 对本机真实 copilot-user-cache.json 做端到端核验：
//! 提取 → 通用 quota_history 落库 → 查询回读。
//! 运行：cargo run -p llm-usage-core --example copilot_quota_probe

use std::collections::BTreeMap;

fn main() {
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(std::path::PathBuf::from);

    let Some(cache) = llm_usage_core::copilot_quota::read_from(&env, home.as_deref()) else {
        println!("VERDICT: FAIL — cache not found or unparseable");
        return;
    };
    let obs = llm_usage_core::copilot_quota::to_observations(&cache);
    for o in &obs {
        println!(
            "  observation: agent={} quota_id={} kind={} unit={} limit={:?} used={:?} remaining={:?} locality_verified={}",
            o.agent, o.quota_id, o.kind, o.unit, o.limit_value, o.used, o.remaining, o.locality_verified
        );
    }

    // 端到端：临时库落库 → 回读（通用 quota_history）。
    let dir = std::path::PathBuf::from("build/copilot-review/quota-e2e");
    std::fs::create_dir_all(&dir).unwrap();
    let storage = llm_usage_core::storage::Storage::open(&dir.join("q.sqlite")).unwrap();
    let now = 1_800_000_000_000i64;
    let n = llm_usage_core::copilot_quota::collect(&storage, &env, home.as_deref(), "UTC", now)
        .unwrap();
    println!("recorded rows = {n}");
    let latest = llm_usage_core::quota_history::latest(&storage, Some("copilot")).unwrap();
    for q in &latest {
        println!(
            "  quota_history: agent={} quota_id={} used={:?} remaining={:?} limit={:?}",
            q.agent, q.quota_id, q.used, q.remaining, q.limit_value
        );
    }
    let series = llm_usage_core::quota_history::daily_series(
        &storage,
        "copilot",
        "premium_interactions",
        "UTC",
    )
    .unwrap();
    println!("daily_series points = {}", series.len());
    let _ = std::fs::remove_dir_all(&dir);

    match latest.iter().find(|q| q.quota_id == "premium_interactions") {
        Some(p) if p.used.is_some() => println!(
            "VERDICT: PASS — end-to-end premium used={} stored+queried",
            p.used.unwrap()
        ),
        _ => println!("VERDICT: no premium_interactions usage found"),
    }
}
