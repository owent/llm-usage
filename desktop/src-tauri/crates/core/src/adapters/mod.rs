//! 来源接入：字段语义映射（V01）、有界 JSONL 读取器（V07）、适配器框架
//! （discover/detect/scan/capability + V12 运行管线）与各 Agent 适配器。
//!
//! 计算规则依据 docs/validation/desktop-usage/m0-agent-fixtures.md 的实读核验结论，
//! 按来源与版本固定，不能由 UI 猜测缺失字段。

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

/// 内置适配器注册表：新增适配器在此登记（目录约定见 architecture.md#adapter-layout）。
/// 应用扫描器与探针工具共用同一注册表。
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
        // 以下为本机未安装产品（2026-09-25 盘点 not_found）：按已核对的文档或源码实现，
        // discover 在本机返回空；真实数据出现后自动发现（真实验收后置）。
        Box::new(cline::ClineAdapter::new()),
        Box::new(dsh::DshAdapter::new()),
        Box::new(hermes::HermesAdapter::new()),
        Box::new(openclaw::OpenClawAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
        Box::new(mimo_code::MimoCodeAdapter::new()),
        Box::new(zoo::ZooAdapter::new()),
        // M8 第二批：最初按 2026-09-29 源码实施；后续隔离容器真实样本
        // 仅提升对应产品/场景的依据，当前范围见各适配器 capability。
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
        // M5：Copilot CLI（本机真实数据核对 2026-09-29）。
        Box::new(copilot::CopilotAdapter::new()),
        // M9：VS Code 内置 Copilot Chat 会话日志（本机真实数据核对 2026-10-01）。
        Box::new(copilot_chat::CopilotChatAdapter::new()),
        // M9：Visual Studio 内置 Copilot 遥测（本机真实数据核对 2026-10-01）。
        Box::new(vs_copilot::VsCopilotAdapter::new()),
        // M5：OTel spans 载体（需启用 exporter/接收器；默认发现仅接收器输出目录）。
        Box::new(otel::OtelAdapter::new()),
    ]
}
