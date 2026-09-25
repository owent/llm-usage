//! 诊断探针：按 app 层相同的 discover 上下文（home/env，无手工根）逐适配器
//! 打印发现根数，用于排查“应用看不到某数据源”类问题。只读，不采集。
use llm_usage_core::adapters::framework::{DiscoverContext, SourceAdapter};

fn main() {
    let adapters: Vec<Box<dyn SourceAdapter>> = vec![
        Box::new(llm_usage_core::adapters::codex::CodexAdapter::new()),
        Box::new(llm_usage_core::adapters::pi::PiAdapter::new()),
        Box::new(llm_usage_core::adapters::omp::OmpAdapter::new()),
        Box::new(llm_usage_core::adapters::kilo::KiloAdapter::new()),
        Box::new(llm_usage_core::adapters::zcode::ZcodeAdapter::new()),
        Box::new(llm_usage_core::adapters::kimi_code::KimiCodeAdapter::new()),
        Box::new(llm_usage_core::adapters::kimi_work::KimiWorkAdapter::new()),
        Box::new(llm_usage_core::adapters::cline::ClineAdapter::new()),
        Box::new(llm_usage_core::adapters::dsh::DshAdapter::new()),
        Box::new(llm_usage_core::adapters::hermes::HermesAdapter::new()),
        Box::new(llm_usage_core::adapters::openclaw::OpenClawAdapter::new()),
    ];
    let ctx = DiscoverContext {
        home_dir: std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(std::path::PathBuf::from),
        env: std::env::vars().collect(),
        manual_roots: vec![],
    };
    println!("home_dir={:?}", ctx.home_dir);
    for a in adapters {
        let roots = a.discover(&ctx);
        println!(
            "{:>10} roots={} {:?}",
            a.adapter_id(),
            roots.len(),
            roots
                .iter()
                .map(|r| r.root.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        );
    }
}
