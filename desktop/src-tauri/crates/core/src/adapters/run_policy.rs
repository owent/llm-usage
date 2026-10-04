//! Cooperative source deadline and retries; parser/permission errors are final.
use crate::error::CoreError;
use std::time::Duration;
use std::{cell::RefCell, sync::Arc, time::Instant};

pub type Allowed = Arc<dyn Fn() -> bool + Send + Sync>;
pub type InstanceAllowed = Arc<dyn Fn(&str) -> bool + Send + Sync>;
#[derive(Clone, Default)]
struct Control {
    deadline: Option<Instant>,
    allowed: Option<Allowed>,
    last_allowed: Option<(Instant, bool)>,
    instance_allowed: Option<InstanceAllowed>,
}
thread_local! {
    static CONTROL: RefCell<Control> = RefCell::new(Control::default());
}
pub struct ControlScope(Control, std::marker::PhantomData<std::rc::Rc<()>>);
impl Drop for ControlScope {
    fn drop(&mut self) {
        CONTROL.with(|current| *current.borrow_mut() = self.0.clone());
    }
}

/// Nest source/file deadlines without leaking control into the next worker job.
pub fn enter(deadline: Option<Instant>, allowed: Option<Allowed>) -> ControlScope {
    CONTROL.with(|current| {
        let previous = current.borrow().clone();
        let deadline = match (previous.deadline, deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let last_allowed = if allowed.is_some() {
            None
        } else {
            previous.last_allowed
        };
        *current.borrow_mut() = Control {
            deadline,
            allowed: allowed.or_else(|| previous.allowed.clone()),
            last_allowed,
            instance_allowed: previous.instance_allowed.clone(),
        };
        ControlScope(previous, std::marker::PhantomData)
    })
}

pub(crate) fn enter_instances(allowed: Option<InstanceAllowed>) {
    CONTROL.with(|current| current.borrow_mut().instance_allowed = allowed);
}
pub fn enter_source(instance: &str, deadline: Option<Instant>) -> ControlScope {
    let current = CONTROL.with(|current| current.borrow().clone());
    let allowed = current.instance_allowed.map(|source_allowed| {
        let instance = instance.to_owned();
        let global = current.allowed;
        Arc::new(move || {
            global.as_ref().map_or(true, |allowed| allowed()) && source_allowed(&instance)
        }) as Allowed
    });
    enter(deadline, allowed)
}

pub fn check() -> Result<(), CoreError> {
    // Do not hold the TLS borrow while invoking a callback.
    let control = CONTROL.with(|current| current.borrow().clone());
    if control
        .deadline
        .is_some_and(|deadline| Instant::now() >= deadline)
    {
        return Err(CoreError::Interrupted("scan_time_budget_exhausted"));
    }
    if let Some(allowed) = control.allowed {
        let now = Instant::now();
        let accepted = match control.last_allowed {
            Some((_, false)) => false,
            Some((at, true)) if now.duration_since(at) < Duration::from_millis(20) => true,
            _ => {
                let accepted = allowed();
                CONTROL.with(|current| current.borrow_mut().last_allowed = Some((now, accepted)));
                accepted
            }
        };
        if !accepted {
            return Err(CoreError::Interrupted("automatic_scan_paused"));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct ReadInterrupted(pub &'static str);
impl std::fmt::Display for ReadInterrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ReadInterrupted {}
pub fn check_io() -> std::io::Result<()> {
    // Read::read_to_end automatically retries ErrorKind::Interrupted. Use a
    // typed Other error so cancellation returns instead of spinning forever.
    check().map_err(|error| match error {
        CoreError::Interrupted(reason) => std::io::Error::other(ReadInterrupted(reason)),
        _ => std::io::Error::other("scan control failed"),
    })
}

pub struct CheckedRead<R>(R);
pub fn checked_reader<R>(reader: R) -> CheckedRead<R> {
    CheckedRead(reader)
}
impl<R: std::io::Read> std::io::Read for CheckedRead<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        check_io()?;
        let length = bytes.len().min(64 * 1024);
        self.0.read(&mut bytes[..length])
    }
}
impl<R: std::io::Seek> std::io::Seek for CheckedRead<R> {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        check_io()?;
        self.0.seek(position)
    }
}
pub fn checked_file(path: &std::path::Path) -> std::io::Result<CheckedRead<std::fs::File>> {
    check_io()?;
    std::fs::File::open(path).map(CheckedRead)
}
pub fn json_from_slice<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, serde_json::Error> {
    let active = CONTROL.with(|current| {
        let current = current.borrow();
        current.deadline.is_some() || current.allowed.is_some()
    });
    if !active {
        return serde_json::from_slice(bytes);
    }
    serde_json::from_reader(std::io::BufReader::with_capacity(
        32 * 1024,
        CheckedRead(std::io::Cursor::new(bytes)),
    ))
}
pub fn json_from_str<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, serde_json::Error> {
    json_from_slice(text.as_bytes())
}

pub fn check_sqlite() -> rusqlite::Result<()> {
    check().map_err(|_| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
            None,
        )
    })
}
pub fn install_sqlite_control(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    connection.progress_handler(1000, Some(|| check().is_err()))
}
/// Only for this app's archive/maintenance connection under its writer lock.
pub struct SqliteScope<'a>(&'a rusqlite::Connection);
impl<'a> SqliteScope<'a> {
    pub fn new(connection: &'a rusqlite::Connection) -> rusqlite::Result<Self> {
        install_sqlite_control(connection)?;
        Ok(Self(connection))
    }
}
impl Drop for SqliteScope<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

pub(super) fn transient(error: &CoreError) -> bool {
    match error {
        CoreError::Io(e) => {
            matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::TimedOut
            ) || cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33))
        }
        CoreError::Sqlite(rusqlite::Error::SqliteFailure(e, _)) => matches!(
            e.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

pub(super) fn retry_delay(
    error: &CoreError,
    retry: usize,
    elapsed: Duration,
    budget: Option<Duration>,
) -> Option<Duration> {
    let delay = Duration::from_secs(*[5, 15, 30].get(retry)?);
    if !transient(error) || budget.is_some_and(|limit| elapsed.saturating_add(delay) >= limit) {
        return None;
    }
    Some(delay)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_classification_attempt_limit_and_deadline_are_independent() {
        let busy = CoreError::Io(std::io::ErrorKind::WouldBlock.into());
        assert_eq!(
            retry_delay(&busy, 0, Duration::ZERO, Some(Duration::from_secs(30))),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            retry_delay(
                &busy,
                1,
                Duration::from_secs(10),
                Some(Duration::from_secs(30))
            ),
            Some(Duration::from_secs(15))
        );
        assert_eq!(
            retry_delay(
                &busy,
                2,
                Duration::from_secs(20),
                Some(Duration::from_secs(30))
            ),
            None
        );
        assert_eq!(
            retry_delay(&busy, 2, Duration::ZERO, None),
            Some(Duration::from_secs(30))
        );
        assert_eq!(retry_delay(&busy, 3, Duration::ZERO, None), None);
        for e in [
            CoreError::Io(std::io::ErrorKind::PermissionDenied.into()),
            CoreError::Validation("bad token".into()),
            CoreError::Json("bad json".into()),
        ] {
            assert_eq!(retry_delay(&e, 0, Duration::ZERO, None), None);
        }
    }
}
