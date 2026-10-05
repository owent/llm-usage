//! Running-process interval waits are monotonic; calendar rules remain UTC based.
use llm_usage_core::{error::CoreError, storage::Storage};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

struct Deadline {
    version: i64,
    updated: i64,
    due: i64,
}
pub struct SourceIntervals {
    epoch: Instant,
    deadlines: BTreeMap<String, Deadline>,
}
impl Default for SourceIntervals {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            deadlines: BTreeMap::new(),
        }
    }
}
impl SourceIntervals {
    pub fn due(&mut self, storage: &Storage, wall: i64) -> Result<BTreeSet<String>, CoreError> {
        self.due_at(
            storage,
            wall,
            i64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(i64::MAX),
        )
    }
    fn due_at(
        &mut self,
        storage: &Storage,
        wall: i64,
        monotonic: i64,
    ) -> Result<BTreeSet<String>, CoreError> {
        let mut stmt=storage.conn().prepare("SELECT e.instance_id,e.rule_kind,e.config_version,e.updated_at_ms,e.next_due_at_ms FROM extraction_schedules e JOIN source_instances s ON s.instance_id=e.instance_id WHERE e.scope='source' AND e.enabled=1 AND s.enabled=1 AND e.next_due_at_ms IS NOT NULL")?;
        let records = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut intervals = BTreeSet::new();
        let mut due = BTreeSet::new();
        for (instance, kind, version, updated, persisted_due) in records {
            if kind != "interval" {
                if persisted_due <= wall {
                    due.insert(instance);
                }
                continue;
            }
            intervals.insert(instance.clone());
            let deadline = self
                .deadlines
                .entry(instance.clone())
                .or_insert_with(|| Deadline {
                    version,
                    updated,
                    due: monotonic.saturating_add(persisted_due.saturating_sub(wall).max(0)),
                });
            if deadline.version != version || deadline.updated != updated {
                *deadline = Deadline {
                    version,
                    updated,
                    due: monotonic.saturating_add(persisted_due.saturating_sub(wall).max(0)),
                };
            }
            if monotonic >= deadline.due {
                due.insert(instance.clone());
            }
            // Keep the UI/headless fallback anchored to this runtime deadline
            // after a wall-clock change, without altering rule revision/state.
            let display_due = wall.saturating_add(deadline.due.saturating_sub(monotonic).max(0));
            if persisted_due.saturating_sub(display_due).unsigned_abs() > 2000 {
                storage.conn().execute("UPDATE extraction_schedules SET next_due_at_ms=?2 WHERE instance_id=?1 AND scope='source' AND config_version=?3 AND updated_at_ms=?4",rusqlite::params![instance,display_due,version,updated])?;
            }
        }
        self.deadlines.retain(|id, _| intervals.contains(id));
        Ok(due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wall_clock_jumps_do_not_fire_intervals_and_completion_rearms() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-execution/source-intervals")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(root.parent().unwrap()).unwrap();
        // PID reuse must not reopen a previous run's disabled schedule.
        std::fs::create_dir(&root).unwrap();
        let storage = Storage::open(&root.join("test.sqlite")).unwrap();
        storage.conn().execute("INSERT OR IGNORE INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,created_at_ms,updated_at_ms) VALUES('a','codex','local_filesystem','verified',1,'ok',0,0)",[]).unwrap();
        let rule = llm_usage_core::schedules::SourceScheduleRule {
            instance_id: "a".into(),
            rule_kind: "interval".into(),
            interval_seconds: Some(60),
            time_of_day: None,
            weekday: None,
            tz: "UTC".into(),
            enabled: true,
        };
        llm_usage_core::schedules::upsert_source_schedule(&storage, &rule, 100000, "UTC").unwrap();
        let mut clock = SourceIntervals::default();
        assert!(clock.due_at(&storage, 100000, 0).unwrap().is_empty());
        assert!(clock.due_at(&storage, 3700000, 10000).unwrap().is_empty());
        assert!(clock.due_at(&storage, 10000, 20000).unwrap().is_empty());
        assert_eq!(
            clock.due_at(&storage, 50000, 60000).unwrap(),
            BTreeSet::from(["a".into()])
        );
        llm_usage_core::schedules::mark_source_run(&storage, "a", 50000, true, "UTC", None)
            .unwrap();
        assert!(clock.due_at(&storage, 50000, 60000).unwrap().is_empty());
        assert_eq!(
            clock.due_at(&storage, 110000, 120000).unwrap(),
            BTreeSet::from(["a".into()])
        );
        storage
            .conn()
            .execute("UPDATE source_instances SET enabled=0", [])
            .unwrap();
        assert!(clock.due_at(&storage, 200000, 200000).unwrap().is_empty());
        assert!(clock.deadlines.is_empty());
        drop(storage);
        std::fs::remove_dir_all(root).unwrap();
    }
}
