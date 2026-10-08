//! Read-only Windows battery-saver status. No system setting is changed.
pub fn battery_saver() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut status = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut status).is_ok() && status.SystemStatusFlag == 1 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn automatic_allowed(interval: u64, pause_on_saver: bool, saver: bool) -> bool {
    interval > 0 && !(pause_on_saver && saver)
}

/// Pending settings stop reads before acquiring the writer mutex. Failure drops
/// only this request; the last persisted settings remain in effect.
pub struct PauseIntent<'a>(&'a std::sync::atomic::AtomicUsize);
impl<'a> PauseIntent<'a> {
    pub fn new(counter: &'a std::sync::atomic::AtomicUsize) -> Self {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(counter)
    }
}
impl Drop for PauseIntent<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn automatic_pause_is_independent_from_manual_refresh() {
        assert!(!super::automatic_allowed(0, false, false));
        assert!(!super::automatic_allowed(3600, true, true));
        assert!(super::automatic_allowed(3600, false, true));
        assert!(super::automatic_allowed(3600, true, false));
    }
}
