mod common;
use common::*;
use llm_usage_core::budgets::*;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::storage::Storage;

fn setup() -> (TempDir, Storage, i64) {
    let (dir, s) = temp_storage("budget");
    let now = ts("2026-10-07T10:00:00Z");
    s.conn().execute("INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,health,created_at_ms,updated_at_ms) VALUES('source','agent-a','local_filesystem','verified','ok',?1,?1)",[now]).unwrap();
    commit_batch(
        &s,
        &batch(
            "source",
            "UTC",
            now,
            vec![
                with_tokens(evt("source", "known", now), 90, 10),
                evt("source", "unknown", now),
            ],
        ),
        None,
    )
    .unwrap();
    (dir, s, now)
}
fn req<'a>(cfg: &'a BudgetSettings, now: i64) -> BudgetRequest<'a> {
    BudgetRequest {
        settings: cfg,
        timezone: "UTC",
        user_id: "default",
        instances: vec!["source".into()],
        now_ms: now,
        pricing_enabled: false,
        price_options: Default::default(),
    }
}

#[test]
fn default_off_unknown_lower_bound_and_persistent_single_notification() {
    let (dir, s, now) = setup();
    let mut cfg = BudgetSettings::default();
    assert!(!claim(&s, &req(&cfg, now)).unwrap().enabled);
    cfg.enabled = true;
    cfg.threshold = "100".into();
    let value = claim(&s, &req(&cfg, now)).unwrap();
    assert_eq!(value.current.as_deref(), Some("100"));
    assert!(value.exceeded && value.newly_triggered && value.coverage_limited);
    assert!(!claim(&s, &req(&cfg, now)).unwrap().newly_triggered);
    drop(s);
    let reopened = Storage::open(&dir.db_path()).unwrap();
    assert!(!claim(&reopened, &req(&cfg, now)).unwrap().newly_triggered);
    cfg.threshold = "99".into();
    assert!(claim(&reopened, &req(&cfg, now)).unwrap().newly_triggered);
    cfg.threshold = "101".into();
    assert!(!claim(&reopened, &req(&cfg, now)).unwrap().exceeded);
}

#[test]
fn local_month_day_dst_and_user_isolation() {
    let (_dir, s, now) = setup();
    let cfg = BudgetSettings {
        enabled: true,
        threshold: "100".into(),
        ..Default::default()
    };
    let mut r = req(&cfg, now);
    r.timezone = "America/New_York";
    let status = evaluate(&s, &r).unwrap();
    assert_eq!(status.first_day, "2026-10-01");
    r.user_id = "empty";
    r.instances = vec![];
    let empty = claim(&s, &r).unwrap();
    assert_eq!(empty.current, None);
    assert!(!empty.exceeded && empty.coverage_limited);
    r.instances = vec!["source".into()];
    r.user_id = "other";
    r.timezone = "UTC";
    assert!(claim(&s, &r).unwrap().newly_triggered);
    assert!(claim(&s, &req(&cfg, now)).unwrap().newly_triggered);
    r.now_ms = ts("2026-11-01T03:30:00Z");
    r.timezone = "America/New_York";
    assert_eq!(evaluate(&s, &r).unwrap().first_day, "2026-10-01");
    r.now_ms = ts("2026-11-01T06:30:00Z");
    assert_eq!(evaluate(&s, &r).unwrap().first_day, "2026-11-01");
}

#[test]
fn costs_stay_unknown_when_disabled_and_invalid_thresholds_rejected() {
    let (_dir, s, now) = setup();
    let mut cfg = BudgetSettings {
        enabled: true,
        metric: BudgetMetric::EstimatedCost,
        threshold: "1".into(),
        ..Default::default()
    };
    let status = claim(&s, &req(&cfg, now)).unwrap();
    assert_eq!(status.current, None);
    assert!(status.coverage_limited && !status.exceeded);
    for invalid in ["0", "-1", "1.5", "9223372036854775808"] {
        cfg.threshold = invalid.into();
        assert!(cfg.validate().is_err());
    }
}

#[test]
fn exact_at_time_costs_select_one_currency_without_repricing() {
    let (_dir, s, now) = setup();
    for (currency, amount) in [("USD", 1), ("CNY", 999)] {
        s.conn().execute("INSERT INTO daily_cost_usage(tz_version,local_day,instance_id,agent,provider_id,model_raw,currency,kind,priced_event_count,unpriced_event_count,partial_event_count,ttl_defaulted_events,total_amount_minor,priced_tokens,known_tokens,price_basis,data_revision) VALUES('UTC','2026-10-07','source','agent-a','prov','m',?1,'estimate_at_time',1,0,0,0,?2,100,100,'[]',1)",rusqlite::params![currency,amount]).unwrap();
    }
    let mut cfg = BudgetSettings {
        enabled: true,
        metric: BudgetMetric::EstimatedCost,
        threshold: "2".into(),
        ..Default::default()
    };
    let mut r = req(&cfg, now);
    r.pricing_enabled = true;
    let status = evaluate(&s, &r).unwrap();
    assert_eq!(status.current.as_deref(), Some("1"));
    assert!(!status.exceeded);
    cfg.threshold = "1".into();
    let mut r = req(&cfg, now);
    r.pricing_enabled = true;
    assert!(claim(&s, &r).unwrap().newly_triggered);
    cfg.threshold = "0001".into();
    let mut r = req(&cfg, now);
    r.pricing_enabled = true;
    assert!(
        !claim(&s, &r).unwrap().newly_triggered,
        "equivalent integer thresholds share the reminder"
    );
    cfg.currency = "EUR".into();
    let mut r = req(&cfg, now);
    r.pricing_enabled = true;
    assert_eq!(evaluate(&s, &r).unwrap().current, None);
}

#[test]
fn failed_notice_transaction_does_not_consume_threshold_and_day_has_own_identity() {
    let (_dir, s, now) = setup();
    let mut cfg = BudgetSettings {
        enabled: true,
        threshold: "100".into(),
        ..Default::default()
    };
    s.conn().execute_batch("CREATE TRIGGER fail_budget_notice BEFORE INSERT ON settings WHEN NEW.key LIKE 'budget_notice:%' BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;").unwrap();
    let revision = s.data_revision().unwrap();
    assert!(claim(&s, &req(&cfg, now)).is_err());
    assert_eq!(s.data_revision().unwrap(), revision);
    assert_eq!(
        s.conn()
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key LIKE 'budget_notice:%'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    s.conn()
        .execute_batch("DROP TRIGGER fail_budget_notice")
        .unwrap();
    assert!(claim(&s, &req(&cfg, now)).unwrap().newly_triggered);
    cfg.period = BudgetPeriod::Day;
    let status = claim(&s, &req(&cfg, now)).unwrap();
    assert_eq!(
        (status.first_day.as_str(), status.last_day.as_str()),
        ("2026-10-07", "2026-10-07")
    );
    assert!(status.newly_triggered);
    assert!(!claim(&s, &req(&cfg, now)).unwrap().newly_triggered);
}
