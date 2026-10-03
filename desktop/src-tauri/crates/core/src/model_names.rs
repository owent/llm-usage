//! Verified model spelling and vendor families; never alias different releases.

pub fn model_key(model: &str) -> String {
    let model = model.trim().to_lowercase();
    // Copilot uses both claude-opus-4.8 and Anthropic's claude-opus-4-8.
    if model.starts_with("claude-") {
        model.replace('.', "-")
    } else {
        model
    }
}

/// Vendor identity is separate from the Agent's billing channel.
pub fn reference_model_key(model: &str) -> String {
    match model_key(model).as_str() {
        // Official Kimi Code model table identifies these IDs as K3. Agents
        // report the serving id either bare (`k3` / `k3-256k`, e.g. Kilo Code /
        // oh-my-pi under a custom Kimi provider) or provider-prefixed. This is an
        // API reference alias only; statistics keep the serving profile.
        "kimi-code/k3" | "kimi-code/k3-256k" | "k3" | "k3-256k" => "kimi-k3".into(),
        _ => model_key(model),
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
    } else {
        &[]
    }
}
