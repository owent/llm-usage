//! Verified model spelling and vendor families; never alias different releases.

#[derive(serde::Deserialize)]
pub struct OfflineModel {
    pub id: String,
    pub aliases: Vec<String>,
    pub provider_id: String,
    pub pricing_available: bool,
    pub source_urls: Vec<String>,
    pub note: String,
}

pub fn offline_model_catalog() -> &'static [OfflineModel] {
    #[derive(serde::Deserialize)]
    struct Catalog {
        models: Vec<OfflineModel>,
    }
    static MODELS: std::sync::OnceLock<Vec<OfflineModel>> = std::sync::OnceLock::new();
    MODELS.get_or_init(|| {
        serde_json::from_str::<Catalog>(include_str!("../prices/models-2026-11-16.json"))
            .map(|c| c.models)
            .expect("validated bundled model references")
    })
}

pub fn model_key(model: &str) -> String {
    // Separator spelling only: retain digits, decimal versions, suffixes and namespaces.
    let model = model
        .trim()
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    // Copilot uses both claude-opus-4.8 and Anthropic's claude-opus-4-8.
    if model.starts_with("claude-") {
        model.replace('.', "-")
    } else {
        model
    }
}

/// Vendor identity is separate from the Agent's billing channel.
pub fn reference_model_key(model: &str) -> String {
    reference_model_key_at(model, i64::MAX)
}

/// A provider-qualified ID can use a user-defined route name. Strip a matching explicit
/// provider, or one Copilot CLI provider-name/model-id prefix; other namespaces remain significant.
pub fn reference_model_key_for(model: &str, provider: Option<&str>, at_ms: i64) -> String {
    let key = model_key(model);
    let bare = key
        .split_once('/')
        .and_then(|(prefix, leaf)| {
            (provider.filter(|p| model_key(p) == prefix).is_some()
                || provider.is_some_and(|p| p.trim().eq_ignore_ascii_case("github-copilot")))
            .then_some(leaf)
        })
        .unwrap_or(&key);
    reference_model_key_at(bare, at_ms)
}

/// Serving profiles can change releases in place. Resolve using the usage date,
/// independently of whether prices are evaluated at that date or today.
pub fn reference_model_key_at(model: &str, occurred_at_ms: i64) -> String {
    let key = model_key(model);
    let model = [
        "openai/",
        "anthropic/",
        "google/",
        "moonshotai/",
        "moonshot/",
        "kimi-code/",
        "kimi-coding/",
        "kimi-for-coding-backup/",
        "tencent/",
        "codebuddy/",
        "alibaba/",
        "alibaba-cn/",
    ]
    .iter()
    .find_map(|prefix| key.strip_prefix(prefix))
    .unwrap_or(&key);
    let model = model_key(model);
    match model.as_str() {
        // Explicit official snapshot identity; never strip arbitrary date suffixes.
        "gpt-5.5-2026-04-23" => "gpt-5.5".into(),
        // Official Kimi Code model table identifies these IDs as K3. Agents
        // report the serving id either bare (`k3` / `k3-256k`, e.g. Kilo Code /
        // oh-my-pi under a custom Kimi provider) or provider-prefixed. This is an
        // API reference alias only; statistics keep the serving profile.
        "k3" | "k3-256k" => "kimi-k3".into(),
        // Official Kimi Code model table (2026-10-24): Pro and Highspeed have separate API rates.
        "k3.1-pro" => "kimi-k3.1-pro".into(),
        "k3.1-pro-highspeed" => "kimi-k3.1-pro-highspeed".into(),
        // User-confirmed serving identity (2026-10-05); price substitution is separate.
        "k28-agent-preview" => "kimi-k2.8-preview".into(),
        // Official Kimi Code release notes: upgraded in place on 2026-09-11.
        // Older records remain unresolved; a future profile change must close this interval.
        "kimi-for-coding" if occurred_at_ms >= 1_789_084_800_000 => "kimi-k2.8-preview".into(),
        // Official CodeBuddy local model catalog (2026-10-03): id -> display name.
        "hy4-preview-f" | "hy-4-preview" => "hy4-preview".into(),
        _ => offline_model_catalog()
            .iter()
            .find(|m| m.aliases.iter().any(|a| a == &model))
            .map(|m| m.id.clone())
            .unwrap_or(model),
    }
}

/// Ordered substitutes for current references; usage identity and saved estimates stay unchanged.
pub fn reference_price_substitutes<'a>(
    reference_model: &'a str,
    provider: Option<&str>,
) -> Vec<&'a str> {
    let mut candidates = Vec::new();
    if let Some(base) = reference_model
        .strip_suffix("-preview")
        .filter(|s| !s.is_empty())
    {
        candidates.push(base);
    }
    if reference_model == "kimi-k2.8-preview" {
        candidates.push("kimi-k2.7-code");
    }
    // Codex selects Luna for API-key approval review; the ChatGPT route's serving identity is opaque.
    if reference_model == "codex-auto-review"
        && provider.is_some_and(|p| p.trim().eq_ignore_ascii_case("openai"))
    {
        candidates.push("gpt-5.6-luna");
    }
    candidates
}

/// Vendor identity is separate from the Agent's billing channel.
pub fn official_providers(model: &str) -> &'static [&'static str] {
    if model.starts_with("gpt-")
        || ["o1", "o3", "o4"]
            .iter()
            .any(|p| model == *p || model.starts_with(&format!("{p}-")))
    {
        &["openai"]
    } else if model.starts_with("claude-") {
        &["anthropic"]
    } else if model.starts_with("gemini-") {
        &["google"]
    } else if model.starts_with("kimi-")
        || model.starts_with("moonshot-")
        || model == "k3"
        || model == "k3-256k"
    {
        &["moonshot", "moonshotai", "moonshotai-cn"]
    } else if model.starts_with("glm-") {
        &["zai", "zhipuai"]
    } else if model.starts_with("deepseek-") {
        &["deepseek"]
    } else if model.starts_with("hy4-") {
        &["tencent", "tencent-cloud"]
    } else if model.starts_with("qwen") {
        &["alibaba", "alibaba-cn"]
    } else {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spelling_and_verified_profiles_are_separate() {
        assert_eq!(model_key(" Claude_Opus 4.8 "), "claude-opus-4-8");
        assert_eq!(model_key("GPT_6.1_sol"), "gpt-6.1-sol");
        assert_ne!(model_key("gpt-6.1-sol"), model_key("gpt-6-1-sol"));
        assert_eq!(reference_model_key("HY 4 Preview"), "hy4-preview");
        assert_eq!(reference_model_key("hy4-preview-f"), "hy4-preview");
        assert_eq!(
            reference_model_key("hy4-preview-unknown"),
            "hy4-preview-unknown"
        );
        assert_eq!(reference_model_key("custom/k3"), "custom/k3");
        assert_eq!(
            reference_model_key("anthropic/Claude_Opus 4.8"),
            "claude-opus-4-8"
        );
        assert_eq!(
            reference_model_key_at("kimi-code/kimi-for-coding", 1_789_084_799_999),
            "kimi-for-coding"
        );
        assert_eq!(
            reference_model_key_at("kimi-code/kimi-for-coding", 1_789_084_800_000),
            "kimi-k2.8-preview"
        );
        assert_eq!(
            reference_model_key_for(
                "ZhiPu AI (Coding Plan)/glm-5.3-flash",
                Some("github-copilot"),
                0,
            ),
            "glm-5.3-flash"
        );
        assert_eq!(
            reference_model_key_for(
                "ZhiPu AI (Coding Plan)/glm-5.3-flash",
                Some("other-provider"),
                0,
            ),
            "zhipu-ai-(coding-plan)/glm-5.3-flash"
        );
    }
}
