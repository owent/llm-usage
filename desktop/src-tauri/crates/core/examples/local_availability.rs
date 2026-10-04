//! Bounded, read-only availability check. Emits counts and version metadata only.
//! Never emits paths, session IDs, source content, or IO error messages.
use llm_usage_core::adapters::framework::{DetectOutcome, DiscoverContext};
fn main() {
    let ctx = DiscoverContext {
        home_dir: std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(Into::into),
        env: std::env::vars().collect(),
        manual_roots: vec![],
    };
    for adapter in llm_usage_core::adapters::built_in_adapters() {
        let roots = adapter.discover(&ctx);
        let mut supported = 0;
        let mut pending = 0;
        let mut unsupported = 0;
        let mut errors = 0;
        let files = roots.iter().map(|r| r.files.len()).sum::<usize>();
        let mut versions = std::collections::BTreeSet::new();
        let mut zed_thread_rows = None;
        for root in &roots {
            for file in root.files.iter().take(3) {
                if adapter.adapter_id() == "zed" {
                    if let Ok(conn) = rusqlite::Connection::open_with_flags(
                        file,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    ) {
                        let _ = conn.busy_timeout(std::time::Duration::from_millis(150));
                        zed_thread_rows = conn
                            .query_row("SELECT COUNT(*) FROM threads", [], |r| r.get::<_, i64>(0))
                            .ok();
                    }
                }
                match adapter.detect(file) {
                    Ok(DetectOutcome::Supported {
                        format,
                        format_version,
                        basis,
                    }) => {
                        supported += 1;
                        versions.insert(format!(
                            "{format}:{}:{}",
                            format_version.unwrap_or_else(|| "missing".into()),
                            basis.as_str()
                        ));
                    }
                    Ok(DetectOutcome::Pending) => pending += 1,
                    Ok(_) => unsupported += 1,
                    Err(_) => errors += 1,
                }
            }
        }
        println!(
            "{}",
            serde_json::json!({"adapter":adapter.adapter_id(),"roots":roots.len(),"files":files,"supported_probes":supported,"pending_probes":pending,"unsupported_probes":unsupported,"read_errors":errors,"versions":versions,"zed_thread_rows":zed_thread_rows})
        );
    }
}
