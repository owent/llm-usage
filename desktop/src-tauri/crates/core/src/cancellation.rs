//! Cooperative SQL cancellation with an atomic boundary before commit.
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const IDLE: u8 = 0;
const RUNNING: u8 = 1;
const CANCELLED: u8 = 2;
const COMMITTING: u8 = 3;

#[derive(Default)]
pub struct OperationControl(AtomicU8);

impl OperationControl {
    pub fn begin(&self) -> bool {
        self.0
            .compare_exchange(IDLE, RUNNING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn cancel(&self) -> bool {
        self.0
            .compare_exchange(RUNNING, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == CANCELLED
    }

    pub fn check(&self) -> Result<(), crate::error::CoreError> {
        if self.is_cancelled() {
            Err(crate::error::CoreError::Validation(
                "operation_cancelled".into(),
            ))
        } else {
            Ok(())
        }
    }

    /// Once this succeeds, cancellation cannot claim to undo the transaction.
    pub fn enter_commit(&self) -> Result<(), crate::error::CoreError> {
        self.0
            .compare_exchange(RUNNING, COMMITTING, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
            .map_err(|_| crate::error::CoreError::Validation("operation_cancelled".into()))
    }

    pub fn finish(&self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }

    pub fn install<'a>(
        self: &Arc<Self>,
        conn: &'a rusqlite::Connection,
    ) -> Result<SqlCancellation<'a>, rusqlite::Error> {
        let control = Arc::clone(self);
        conn.progress_handler(1000, Some(move || control.is_cancelled()))?;
        Ok(SqlCancellation(conn))
    }
}

pub struct SqlCancellation<'a>(&'a rusqlite::Connection);
impl Drop for SqlCancellation<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_and_commit_have_one_winner_and_next_operation_resets() {
        let control = OperationControl::default();
        assert!(control.begin());
        assert!(!control.begin());
        assert!(control.cancel());
        assert!(control.enter_commit().is_err());
        control.finish();
        assert!(control.begin());
        control.enter_commit().unwrap();
        assert!(!control.cancel());
        control.finish();
        assert!(control.begin());
        assert!(!control.is_cancelled());
    }

    #[test]
    fn cancellation_rolls_back_sql_and_removes_the_progress_handler() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(value); INSERT INTO t VALUES(1)")
            .unwrap();
        let control = Arc::new(OperationControl::default());
        assert!(control.begin());
        let sql = control.install(&conn).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute("DELETE FROM t", []).unwrap();
        assert!(control.cancel());
        assert!(tx.execute_batch("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<100000) INSERT INTO t SELECT x FROM n").is_err());
        drop(tx);
        drop(sql);
        assert!(conn.is_autocommit());
        assert_eq!(
            conn.query_row("SELECT SUM(value) FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        control.finish();
        conn.execute("INSERT INTO t VALUES(2)", []).unwrap();
    }
}
