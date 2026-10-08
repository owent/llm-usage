//! Verify Storage::open_readonly behavior for a database with a WAL.
use llm_usage_core::storage::Storage;
fn main() {
    let db = std::env::args().nth(1).expect("db path");
    match Storage::open_readonly(std::path::Path::new(&db)) {
        Ok(s) => {
            let n: i64 = s
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
                .unwrap();
            println!("readonly ok: {n} events");
        }
        Err(e) => println!("readonly ERR: {e}"),
    }
}
