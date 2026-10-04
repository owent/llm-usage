//! Bounded connection-local summaries. Stamps are compared only on one connection.
use crate::query::Summary;
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 4;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub revision: i64,
    pub data_version: i64,
    pub total_changes: u64,
}
struct Entry {
    key: String,
    value: Summary,
    bytes: usize,
}
#[derive(Default)]
pub(crate) struct SummaryCache {
    stamp: Option<Stamp>,
    entries: VecDeque<Entry>,
    bytes: usize,
}
impl SummaryCache {
    pub(crate) fn get(&mut self, stamp: Stamp, key: &str) -> Option<Summary> {
        if self.stamp != Some(stamp) {
            self.clear();
            self.stamp = Some(stamp);
        }
        let position = self.entries.iter().position(|entry| entry.key == key)?;
        let entry = self.entries.remove(position)?;
        let value = entry.value.clone();
        self.entries.push_back(entry);
        Some(value)
    }
    pub(crate) fn put(&mut self, key: String, value: &Summary) {
        if retained_bytes(&key, value) > MAX_BYTES {
            return;
        }
        let value = value.clone();
        let bytes = retained_bytes(&key, &value);
        if bytes > MAX_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            if let Some(entry) = self.entries.pop_front() {
                self.bytes -= entry.bytes;
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.entries.push_back(Entry { key, value, bytes });
    }
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
        self.stamp = None;
    }
}

fn retained_bytes(key: &String, value: &Summary) -> usize {
    std::mem::size_of::<Entry>()
        + key.capacity()
        + value.timezone.capacity()
        + value.periods.capacity() * std::mem::size_of::<crate::query::PeriodRow>()
        + value
            .periods
            .iter()
            .map(|row| row.label.capacity())
            .sum::<usize>()
        + value.model_breakdown.capacity() * std::mem::size_of::<crate::query::ModelRow>()
        + value
            .model_breakdown
            .iter()
            .map(|row| {
                row.provider_id.as_ref().map_or(0, String::capacity)
                    + row.model_raw.as_ref().map_or(0, String::capacity)
            })
            .sum::<usize>()
        + value.agent_breakdown.capacity() * std::mem::size_of::<crate::query::AgentRow>()
        + value
            .agent_breakdown
            .iter()
            .map(|row| row.agent.capacity())
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn summary() -> Summary {
        Summary {
            data_revision: 1,
            timezone: "UTC".into(),
            week_start: crate::calendar::WeekStart::Monday,
            periods: vec![],
            totals: Default::default(),
            model_breakdown: vec![],
            agent_breakdown: vec![],
            distinct_sessions: Some(0),
            active_days: Some(0),
            excluded_event_count: 0,
        }
    }
    #[test]
    fn entries_are_bounded_recently_used_and_results_are_independent() {
        let stamp = Stamp {
            revision: 1,
            data_version: 1,
            total_changes: 0,
        };
        let mut cache = SummaryCache::default();
        assert!(cache.get(stamp, "0").is_none());
        for key in 0..4 {
            cache.put(key.to_string(), &summary());
        }
        let mut returned = cache.get(stamp, "0").unwrap();
        returned.totals.call_count = 999;
        cache.put("4".into(), &summary());
        assert!(cache.get(stamp, "1").is_none());
        assert_eq!(cache.get(stamp, "0").unwrap().totals.call_count, 0);
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache.bytes <= MAX_BYTES);
        assert!(cache
            .get(
                Stamp {
                    total_changes: 1,
                    ..stamp
                },
                "0"
            )
            .is_none());
        assert_eq!(cache.bytes, 0);
    }
    #[test]
    fn byte_budget_evicts_old_values_and_oversized_values_are_bypassed() {
        let stamp = Stamp {
            revision: 1,
            data_version: 1,
            total_changes: 0,
        };
        let mut cache = SummaryCache::default();
        cache.get(stamp, "first");
        let large = "x".repeat(MAX_BYTES / 2 + 1000);
        let value = Summary {
            timezone: large,
            ..summary()
        };
        cache.put("first".into(), &value);
        cache.put("second".into(), &value);
        assert!(cache.get(stamp, "first").is_none());
        assert!(cache.get(stamp, "second").is_some());
        cache.put("x".repeat(MAX_BYTES), &summary());
        assert!(cache.get(stamp, "second").is_some());
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.bytes <= MAX_BYTES);
    }
}
