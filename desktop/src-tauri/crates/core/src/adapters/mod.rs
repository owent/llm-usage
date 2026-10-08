//! Source integration: field mappings (V01), bounded JSONL reading (V07), adapter framework
//! discovery/detection/scanning/capabilities and V12 ingestion, plus product adapters.
//!
//! Native checks in docs/validation/desktop-usage/m0-agent-fixtures.md define calculation rules
//! per source/version; the UI cannot infer missing fields.

pub mod aider;
pub mod amp;
pub mod antigravity;
pub mod atomcode;
pub mod claude;
pub mod cline;
pub mod codebuddy;
pub mod codex;
pub mod commandcode;
#[path = "continue/mod.rs"]
pub mod continuedev;
pub mod copilot;
pub mod copilot_chat;
pub mod crush;
pub mod droid;
pub mod dsh;
pub mod framework;
#[path = "gajae-code/mod.rs"]
pub mod gajae_code;
pub mod gemini;
pub mod goose;
pub mod grok;
pub mod hermes;
pub mod jcode;
pub mod jsonl;
pub mod junie;
pub mod kilo;
#[path = "kimi-code/mod.rs"]
pub mod kimi_code;
pub mod kimi_wire;
#[path = "kimi-work/mod.rs"]
pub mod kimi_work;
pub mod kiro;
#[path = "mimo-code/mod.rs"]
pub mod mimo_code;
pub mod omp;
pub mod openclaw;
pub mod opencode;
pub mod opencode_family;
pub mod otel;
pub mod pi;
pub mod qoder;
pub mod qwen;
pub mod roo;
pub mod routing;
pub mod run_policy;
pub mod tencent_buddy_wire;
pub mod usage_map;
pub mod vs_copilot;
pub mod workbuddy;
pub mod xum;
pub mod zcode;
pub mod zed;
pub mod zoo;

/// Register built-in adapters here; directory rules are in architecture.md#adapter-layout.
/// Application scanners and probe tools use this same registry.
pub fn built_in_adapters() -> Vec<Box<dyn framework::SourceAdapter>> {
    vec![
        Box::new(codex::CodexAdapter::new()),
        Box::new(claude::ClaudeAdapter::new()),
        Box::new(pi::PiAdapter::new()),
        Box::new(omp::OmpAdapter::new()),
        Box::new(gemini::GeminiAdapter::new()),
        Box::new(qwen::QwenAdapter::new()),
        Box::new(kilo::KiloAdapter::new()),
        Box::new(zcode::ZcodeAdapter::new()),
        Box::new(kimi_code::KimiCodeAdapter::new()),
        Box::new(kimi_work::KimiWorkAdapter::new()),
        // The original 2026-09-25 inventory found these products absent; implementation began from checked docs/source.
        // Discovery returned no local roots at that stage; later native checks have product-specific scopes.
        Box::new(cline::ClineAdapter::new()),
        Box::new(dsh::DshAdapter::new()),
        Box::new(hermes::HermesAdapter::new()),
        Box::new(openclaw::OpenClawAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
        Box::new(mimo_code::MimoCodeAdapter::new()),
        Box::new(zoo::ZooAdapter::new()),
        // M8 second batch began from 2026-09-29 source checks. Later isolated-container samples
        // verify only the respective product/scenario; each capability records the current scope.
        Box::new(zed::ZedAdapter::new()),
        Box::new(aider::AiderAdapter::new()),
        Box::new(junie::JunieAdapter::new()),
        Box::new(xum::XumAdapter::new()),
        Box::new(droid::DroidAdapter::new()),
        Box::new(amp::AmpAdapter::new()),
        Box::new(grok::GrokAdapter::new()),
        Box::new(roo::RooAdapter::new()),
        Box::new(goose::GooseAdapter::new()),
        Box::new(crush::CrushAdapter::new()),
        Box::new(jcode::JcodeAdapter::new()),
        Box::new(codebuddy::CodeBuddyAdapter::new()),
        Box::new(workbuddy::WorkBuddyAdapter::new()),
        Box::new(gajae_code::GajaeCodeAdapter::new()),
        Box::new(commandcode::CommandCodeAdapter::new()),
        Box::new(continuedev::ContinueAdapter::new()),
        Box::new(atomcode::AtomCodeAdapter::new()),
        Box::new(kiro::KiroAdapter::new()),
        Box::new(antigravity::AntigravityAdapter::new()),
        Box::new(qoder::QoderAdapter::new()),
        // M5 Copilot CLI: native local data checked 2026-09-29.
        Box::new(copilot::CopilotAdapter::new()),
        // M9 VS Code built-in Copilot Chat sessions: native local data checked 2026-10-01.
        Box::new(copilot_chat::CopilotChatAdapter::new()),
        // M9 Visual Studio built-in Copilot telemetry: native local data checked 2026-10-01.
        Box::new(vs_copilot::VsCopilotAdapter::new()),
        // M5 OTel span files require an exporter/receiver; default discovery reads receiver output directories only.
        Box::new(otel::OtelAdapter::new()),
    ]
}
