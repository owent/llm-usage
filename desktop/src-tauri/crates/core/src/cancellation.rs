//! Cooperative SQL cancellation with an atomic boundary before commit.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const IDLE: u8 = 0;
const RUNNING: u8 = 1;
const CANCELLED: u8 = 2;
const COMMITTING: u8 = 3;

pub(crate) struct SqlInterrupt {
    stop: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) interrupted: Arc<AtomicBool>,
    #[cfg(test)]
    called: Arc<(Mutex<u64>, Condvar)>,
}

pub(crate) fn interrupt_on(
    connection: &rusqlite::Connection,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Result<SqlInterrupt, crate::error::CoreError> {
    let interrupt = connection.get_interrupt_handle();
    let interrupted = Arc::new(AtomicBool::new(false));
    let worker_interrupted = Arc::clone(&interrupted);
    #[cfg(test)]
    let called = Arc::new((Mutex::new(0), Condvar::new()));
    #[cfg(test)]
    let worker_called = Arc::clone(&called);
    let stop = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_stop = Arc::clone(&stop);
    let worker = std::thread::Builder::new()
        .name("sqlite-cancel".into())
        .spawn(move || {
            let (lock, wake) = &*worker_stop;
            loop {
                if cancelled() {
                    worker_interrupted.store(true, Ordering::SeqCst);
                    #[cfg(test)]
                    let mut calls = worker_called.0.lock().unwrap();
                    interrupt.interrupt();
                    #[cfg(test)]
                    {
                        *calls += 1;
                        worker_called.1.notify_all();
                    }
                }
                let guard = lock.lock().unwrap();
                let (stopped, _) = wake.wait_timeout(guard, Duration::from_millis(20)).unwrap();
                if *stopped {
                    break;
                }
            }
        })
        .map_err(crate::error::CoreError::Io)?;
    Ok(SqlInterrupt {
        stop,
        worker: Some(worker),
        interrupted,
        #[cfg(test)]
        called,
    })
}

impl SqlInterrupt {
    pub(crate) fn stop(&mut self) {
        let (lock, wake) = &*self.stop;
        *lock.lock().unwrap() = true;
        wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SqlInterrupt {
    fn drop(&mut self) {
        self.stop();
    }
}

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
    ) -> Result<SqlCancellation<'a>, crate::error::CoreError> {
        let control = Arc::clone(self);
        Ok(SqlCancellation {
            _connection: std::marker::PhantomData,
            _worker: interrupt_on(conn, Arc::new(move || control.is_cancelled()))?,
        })
    }
}

pub struct SqlCancellation<'a> {
    _connection: std::marker::PhantomData<&'a rusqlite::Connection>,
    _worker: SqlInterrupt,
}

#[cfg(test)]
mod tests {
    use super::*;

    // The recursive step checks the interrupt after the blocking function returns.
    const INSERT_WITH_CANCELLATION: &str = "WITH RECURSIVE n(x) AS (
        SELECT wait_for_cancellation() UNION ALL SELECT x+1 FROM n WHERE x<10
    ) INSERT INTO t SELECT x FROM n";

    fn wait_for_interrupt(called: &(Mutex<u64>, Condvar), previous: u64) {
        let (calls, _) = called
            .1
            .wait_timeout_while(called.0.lock().unwrap(), Duration::from_secs(2), |calls| {
                *calls <= previous
            })
            .unwrap();
        assert!(
            *calls > previous,
            "SQL cancellation worker did not interrupt"
        );
    }

    fn register_cancellation_barrier(
        conn: &rusqlite::Connection,
        control: &Arc<OperationControl>,
        sql: &SqlCancellation<'_>,
    ) {
        let control = Arc::clone(control);
        let called = Arc::clone(&sql._worker.called);
        conn.create_scalar_function(
            "wait_for_cancellation",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                // Hold the statement open until the worker interrupts this running SQL.
                let calls = called.0.lock().unwrap();
                let previous = *calls;
                assert!(control.is_cancelled() || control.cancel());
                let (calls, _) = called
                    .1
                    .wait_timeout_while(calls, Duration::from_secs(2), |calls| *calls <= previous)
                    .unwrap();
                assert!(
                    *calls > previous,
                    "SQL cancellation worker did not interrupt"
                );
                Ok(2)
            },
        )
        .unwrap();
    }

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
    fn cancellation_before_sql_interrupts_a_later_statement() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(value)").unwrap();
        let control = Arc::new(OperationControl::default());
        assert!(control.begin());
        let sql = control.install(&conn).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute("INSERT INTO t VALUES(1)", []).unwrap();
        assert!(control.cancel());
        wait_for_interrupt(&sql._worker.called, 0);
        register_cancellation_barrier(&conn, &control, &sql);
        let error = tx.execute(INSERT_WITH_CANCELLATION, []).unwrap_err();
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::OperationInterrupted)
        );
        assert!(tx.is_autocommit());
        drop(tx);
        drop(sql);
        assert!(conn.is_autocommit());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        control.finish();
        conn.execute("INSERT INTO t VALUES(1)", []).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn cancellation_rolls_back_sql_and_stops_the_interrupt_worker() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(value); INSERT INTO t VALUES(1)")
            .unwrap();
        let control = Arc::new(OperationControl::default());
        assert!(control.begin());
        let sql = control.install(&conn).unwrap();
        register_cancellation_barrier(&conn, &control, &sql);
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute("DELETE FROM t", []).unwrap();
        let error = tx.execute(INSERT_WITH_CANCELLATION, []).unwrap_err();
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::OperationInterrupted)
        );
        assert!(tx.is_autocommit());
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
