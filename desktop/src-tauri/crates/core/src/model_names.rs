//! Verified model spelling and vendor families; never alias different releases.

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

/// A provider-qualified ID can use a user-defined route name. Only strip a
/// matching explicit provider; unrelated namespaces remain significant.
pub fn reference_model_key_for(model: &str, provider: Option<&str>, at_ms: i64) -> String {
    let key = model_key(model);
    let bare = key
        .split_once('/')
        .and_then(|(prefix, leaf)| provider.filter(|p| model_key(p) == prefix).map(|_| leaf))
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
        // User-confirmed serving identity (2026-10-05); price substitution is separate.
        "k28-agent-preview" => "kimi-k2.8-preview".into(),
        // Official Kimi Code release notes: upgraded in place on 2026-09-11.
        // Older records remain unresolved; a future profile change must close this interval.
        "kimi-for-coding" if occurred_at_ms >= 1_789_084_800_000 => "kimi-k2.8-preview".into(),
        // Official CodeBuddy local model catalog (2026-10-03): id -> display name.
        "hy4-preview-f" | "hy-4-preview" => "hy4-preview".into(),
        _ => model,
    }
}

/// Explicit user-authorized substitute for CURRENT reference estimates only.
/// Preserve model identity; this exception does not permit general nearest-model pricing.
pub fn reference_price_substitute(reference_model: &str) -> Option<&'static str> {
    match reference_model {
        "kimi-k2.8-preview" => Some("kimi-k2.7-code"),
        _ => None,
    }
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
    }
}
