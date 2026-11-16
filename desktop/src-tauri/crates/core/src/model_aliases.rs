//! Versioned display metadata, independent of usage identity, grouping and prices.

use crate::{model_names::model_key, storage::Storage, CoreError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const STATE_KEY: &str = "model_alias_catalog_v1";
pub const MAX_CATALOG_BYTES: usize = 1_048_576;
const MAX_STATE_BYTES: usize = 33_554_432;
const MIN_MS: i64 = crate::domain::MIN_PLAUSIBLE_MS;
const MAX_MS: i64 = 4_102_444_800_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub rule_version: String,
    pub published_at: String,
    pub aliases: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub agent: String,
    pub key: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub display_name: String,
    pub effective_from: String,
    #[serde(default)]
    pub effective_until: Option<String>,
    pub basis: String,
    #[serde(default)]
    pub source_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub agent: String,
    pub key: String,
    pub provider_id: Option<String>,
    pub display_name: String,
    pub from_ms: i64,
    pub until_ms: Option<i64>,
    pub rule_version: String,
    pub content_hash: String,
    pub origin: String,
    pub basis: String,
    pub source_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedCatalog {
    pub raw: String,
    pub content_hash: String,
    pub fetched_at_ms: i64,
    pub published_at_ms: i64,
    pub rule_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema_version: u32,
    pub generation: u64,
    pub versions: BTreeMap<String, String>,
    pub candidates: Vec<Candidate>,
    pub cache: Option<CachedCatalog>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: 1,
            generation: 0,
            versions: BTreeMap::new(),
            candidates: Vec::new(),
            cache: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    content_hash: String,
    state: State,
}

fn invalid(code: &str) -> CoreError {
    CoreError::Validation(format!("model_alias_{code}"))
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn scope(value: &str) -> bool {
    text(value, 160)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_/.:".contains(&b))
}

fn timestamp(value: &str) -> Result<i64, CoreError> {
    if !value.ends_with('Z') || value.len() > 35 {
        return Err(invalid("timestamp"));
    }
    let at = value
        .parse::<jiff::Timestamp>()
        .map_err(|_| invalid("timestamp"))?
        .as_millisecond();
    if !(MIN_MS..MAX_MS).contains(&at) {
        return Err(invalid("timestamp"));
    }
    Ok(at)
}

fn public_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    text(value, 2048)
        && !value.contains('?')
        && !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
}

/// Invalid input returns bounded codes, never catalogue values or parser excerpts.
pub fn parse_catalog(raw: &str, now_ms: i64) -> Result<Catalog, CoreError> {
    if raw.is_empty() || raw.len() > MAX_CATALOG_BYTES {
        return Err(invalid("size"));
    }
    let catalog: Catalog = serde_json::from_str(raw).map_err(|_| invalid("json"))?;
    if catalog.schema_version != 1
        || !text(&catalog.rule_version, 128)
        || catalog.aliases.is_empty()
        || catalog.aliases.len() > 2_000
    {
        return Err(invalid("schema"));
    }
    let published = timestamp(&catalog.published_at)?;
    if published > now_ms.saturating_add(300_000) {
        return Err(invalid("future_publication"));
    }
    let mut unique = BTreeSet::new();
    for rule in &catalog.aliases {
        let from = timestamp(&rule.effective_from)?;
        let until = rule.effective_until.as_deref().map(timestamp).transpose()?;
        if !scope(&rule.agent)
            || !text(&rule.key, 160)
            || !text(&rule.display_name, 160)
            || rule.provider_id.as_deref().is_some_and(|p| !scope(p))
            || !matches!(
                rule.basis.as_str(),
                "user_override" | "catalogue" | "native_snapshot"
            )
            || until.is_some_and(|end| end <= from)
            || rule.source_urls.len() > 8
            || rule.source_urls.iter().any(|url| !public_url(url))
        {
            return Err(invalid("rule"));
        }
        if !unique.insert((
            rule.agent.to_lowercase(),
            model_key(&rule.key),
            rule.provider_id.as_ref().map(|p| p.to_lowercase()),
            from,
            until,
        )) {
            return Err(invalid("duplicate"));
        }
    }
    Ok(catalog)
}

pub fn read_state(storage: &Storage) -> Result<(State, String), CoreError> {
    let raw: Option<String> = storage.conn().query_row(
        "SELECT CASE WHEN length(CAST(value AS BLOB)) <= ?2 THEN value ELSE NULL END FROM settings WHERE key=?1",
        params![STATE_KEY, MAX_STATE_BYTES as i64], |row| row.get(0),
    ).optional()?.flatten();
    if let Some(raw) = raw {
        let envelope: Envelope = serde_json::from_str(&raw).map_err(|_| invalid("state"))?;
        let serialized = serde_json::to_vec(&envelope.state).map_err(|_| invalid("state"))?;
        if digest(&serialized) != envelope.content_hash {
            return Err(invalid("state_integrity"));
        }
        let state = envelope.state;
        if state.schema_version != 1
            || state.candidates.len() > 20_000
            || state.versions.len() > 4_096
        {
            return Err(invalid("state"));
        }
        for c in &state.candidates {
            if !scope(&c.agent)
                || !text(&c.key, 160)
                || !text(&c.display_name, 160)
                || c.provider_id.as_deref().is_some_and(|p| !scope(p))
                || !(MIN_MS..MAX_MS).contains(&c.from_ms)
                || c.until_ms.is_some_and(|to| to <= c.from_ms || to >= MAX_MS)
                || state.versions.get(&c.rule_version) != Some(&c.content_hash)
                || !matches!(c.origin.as_str(), "online" | "manual")
                || !matches!(
                    c.basis.as_str(),
                    "user_override" | "catalogue" | "native_snapshot"
                )
                || c.source_urls.len() > 8
                || c.source_urls.iter().any(|url| !public_url(url))
            {
                return Err(invalid("state"));
            }
        }
        if let Some(cache) = &state.cache {
            if digest(cache.raw.as_bytes()) != cache.content_hash
                || cache.raw.len() > MAX_CATALOG_BYTES
                || state.versions.get(&cache.rule_version) != Some(&cache.content_hash)
            {
                return Err(invalid("cache_integrity"));
            }
            let parsed = parse_catalog(&cache.raw, cache.fetched_at_ms)?;
            if parsed.rule_version != cache.rule_version
                || timestamp(&parsed.published_at)? != cache.published_at_ms
            {
                return Err(invalid("cache_integrity"));
            }
        }
        Ok((state, digest(raw.as_bytes())))
    } else {
        let exists: bool = storage.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",
            [STATE_KEY],
            |r| r.get(0),
        )?;
        if exists {
            Err(invalid("state_size"))
        } else {
            Ok((State::default(), "missing".into()))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportOutcome {
    pub generation: u64,
    pub inserted_rules: usize,
    pub already_present: bool,
    pub content_hash: String,
}

/// Compare the complete state token in the same writer transaction as the import/cache update.
pub fn import_catalog(
    storage: &Storage,
    raw: &str,
    now_ms: i64,
    origin: &str,
    expected_etag: Option<&str>,
    update_cache: bool,
) -> Result<ImportOutcome, CoreError> {
    import_inner(
        storage,
        raw,
        now_ms,
        ImportOptions {
            origin,
            expected_etag,
            update_cache,
            settings_stamp: None,
        },
    )
}

struct ImportOptions<'a> {
    origin: &'a str,
    expected_etag: Option<&'a str>,
    update_cache: bool,
    settings_stamp: Option<i64>,
}

pub fn settings_stamp(storage: &Storage) -> Result<i64, CoreError> {
    Ok(storage
        .conn()
        .query_row(
            "SELECT updated_at_ms FROM settings WHERE key='app_settings'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

pub fn import_online_catalog(
    storage: &Storage,
    raw: &str,
    now_ms: i64,
    etag: &str,
    stamp: i64,
) -> Result<ImportOutcome, CoreError> {
    import_inner(
        storage,
        raw,
        now_ms,
        ImportOptions {
            origin: "online",
            expected_etag: Some(etag),
            update_cache: true,
            settings_stamp: Some(stamp),
        },
    )
}

fn import_inner(
    storage: &Storage,
    raw: &str,
    now_ms: i64,
    options: ImportOptions<'_>,
) -> Result<ImportOutcome, CoreError> {
    let ImportOptions {
        origin,
        expected_etag,
        update_cache,
        settings_stamp: expected_settings,
    } = options;
    let catalog = parse_catalog(raw, now_ms)?;
    if !matches!(origin, "manual" | "online") {
        return Err(invalid("origin"));
    }
    let hash = digest(raw.as_bytes());
    let published = timestamp(&catalog.published_at)?;
    let tx = storage.conn().unchecked_transaction()?;
    if expected_settings.is_some_and(|expected| settings_stamp(storage).ok() != Some(expected)) {
        return Err(invalid("stale_worker"));
    }
    let (mut state, etag) = read_state(storage)?;
    if expected_etag.is_some_and(|expected| expected != etag) {
        return Err(invalid("stale_worker"));
    }
    if update_cache
        && state
            .cache
            .as_ref()
            .is_some_and(|c| published < c.published_at_ms)
    {
        return Err(invalid("stale_publication"));
    }
    let known = state.versions.get(&catalog.rule_version);
    if known.is_some_and(|previous| previous != &hash) {
        return Err(invalid("version_conflict"));
    }
    let already_present = known.is_some();
    let inserted = if already_present {
        0
    } else {
        catalog.aliases.len()
    };
    if !already_present {
        for rule in &catalog.aliases {
            state.candidates.push(Candidate {
                agent: rule.agent.to_lowercase(),
                key: model_key(&rule.key),
                provider_id: rule.provider_id.as_ref().map(|p| p.to_lowercase()),
                display_name: rule.display_name.clone(),
                from_ms: timestamp(&rule.effective_from)?,
                until_ms: rule.effective_until.as_deref().map(timestamp).transpose()?,
                rule_version: catalog.rule_version.clone(),
                content_hash: hash.clone(),
                origin: origin.into(),
                basis: rule.basis.clone(),
                source_urls: rule.source_urls.clone(),
            });
        }
        state
            .versions
            .insert(catalog.rule_version.clone(), hash.clone());
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("generation"))?;
    }
    if state.candidates.len() > 20_000 || state.versions.len() > 4_096 {
        return Err(invalid("history_limit"));
    }
    if update_cache {
        state.cache = Some(CachedCatalog {
            raw: raw.into(),
            content_hash: hash.clone(),
            fetched_at_ms: now_ms,
            published_at_ms: published,
            rule_version: catalog.rule_version,
        });
    }
    let state_json = serde_json::to_vec(&state).map_err(|_| invalid("serialize"))?;
    let json = serde_json::to_string(&Envelope {
        content_hash: digest(&state_json),
        state: state.clone(),
    })
    .map_err(|_| invalid("serialize"))?;
    if json.len() > MAX_STATE_BYTES {
        return Err(invalid("history_limit"));
    }
    storage.conn().execute(
        "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
        ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
        params![STATE_KEY, json, now_ms],
    )?;
    tx.commit()?;
    Ok(ImportOutcome {
        generation: state.generation,
        inserted_rules: inserted,
        already_present,
        content_hash: hash,
    })
}

#[derive(Debug, Clone)]
pub struct Book {
    pub revision: String,
    pub error: Option<String>,
    candidates: Vec<Candidate>,
}

/// Validate metadata before publishing its revision to UI status polling.
pub fn revision(storage: &Storage) -> String {
    match read_state(storage) {
        Ok((state, _)) => format!("model-display-2026-11-16:{}", state.generation),
        Err(_) => "model-alias-invalid".into(),
    }
}

impl Book {
    /// Corrupt display metadata cannot prevent usage queries or revive stale cached names.
    pub fn load(storage: &Storage) -> Self {
        match read_state(storage) {
            Ok((state, _)) => {
                let mut candidates = bundled_candidates();
                candidates.extend(state.candidates);
                Self {
                    revision: format!("model-display-2026-11-16:{}", state.generation),
                    error: None,
                    candidates,
                }
            }
            Err(_) => Self {
                revision: "model-alias-invalid".into(),
                error: Some("model_alias_state".into()),
                candidates: Vec::new(),
            },
        }
    }

    pub fn contains(&self, raw: &str) -> bool {
        let key = model_key(raw);
        self.candidates.iter().any(|c| c.key == key)
    }

    pub fn keys(&self) -> Vec<String> {
        self.candidates
            .iter()
            .map(|c| c.key.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// One label must cover the complete half-open range; gaps/conflicts retain the raw identity.
    pub fn label(
        &self,
        raw: &str,
        agent: &str,
        provider: Option<&str>,
        from: i64,
        to: i64,
    ) -> String {
        let key = model_key(raw);
        if from >= to {
            return raw.into();
        }
        let provider = provider.filter(|p| !p.is_empty()).map(str::to_lowercase);
        let rules: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| {
                c.key == key
                    && c.agent.eq_ignore_ascii_case(agent)
                    && (c.provider_id.is_none() || c.provider_id == provider)
            })
            .collect();
        if rules.is_empty() {
            return raw.into();
        }
        let mut groups: BTreeMap<(bool, i64), (BTreeSet<String>, i64)> = BTreeMap::new();
        for c in &rules {
            let group = groups
                .entry((c.provider_id.is_some(), c.from_ms))
                .or_insert_with(|| (BTreeSet::new(), MAX_MS));
            group.0.insert(c.display_name.clone());
            group.1 = group.1.min(c.until_ms.unwrap_or(MAX_MS));
        }
        let Some(mut latest) = groups
            .keys()
            .filter(|(_, start)| *start <= from)
            .max()
            .copied()
        else {
            return raw.into();
        };
        let label = groups[&latest].0.first().expect("nonempty group").clone();
        let valid = |priority, end| {
            let group = &groups[&priority];
            group.0.len() == 1 && group.0.first() == Some(&label) && group.1 >= end
        };
        let mut changes: BTreeMap<i64, (bool, i64)> = BTreeMap::new();
        for &priority in groups
            .keys()
            .filter(|(_, start)| *start > from && *start < to)
        {
            changes
                .entry(priority.1)
                .and_modify(|p| *p = (*p).max(priority))
                .or_insert(priority);
        }
        for (at, priority) in changes {
            if priority > latest {
                if !valid(latest, at) {
                    return raw.into();
                }
                latest = priority;
            }
        }
        if valid(latest, to) {
            label
        } else {
            raw.into()
        }
    }
}

fn bundled_candidates() -> Vec<Candidate> {
    #[derive(Deserialize)]
    struct Seed {
        rule_version: String,
        aliases: Vec<SeedRule>,
    }
    #[derive(Deserialize)]
    struct SeedRule {
        key: String,
        agent: String,
        display_name: String,
        basis: String,
    }
    static SEED: std::sync::OnceLock<Vec<Candidate>> = std::sync::OnceLock::new();
    SEED.get_or_init(|| {
        let raw = include_str!("../../../../src/lib/model-display-aliases.json");
        let seed: Seed = serde_json::from_str(raw).expect("valid bundled display aliases");
        seed.aliases
            .into_iter()
            .map(|c| Candidate {
                agent: c.agent,
                key: model_key(&c.key),
                provider_id: None,
                display_name: c.display_name,
                from_ms: MIN_MS,
                until_ms: None,
                rule_version: seed.rule_version.clone(),
                content_hash: digest(raw.as_bytes()),
                origin: "bundled".into(),
                basis: c.basis,
                source_urls: Vec::new(),
            })
            .collect()
    })
    .clone()
}
