//! 来源接入：字段口径映射（V01）、有界 JSONL 读取器（V07）、适配器框架
//! （discover/detect/scan/capability + V12 运行管线）与各 Agent 适配器。
//!
//! 口径依据 docs/validation/desktop-usage/m0-agent-fixtures.md 的实读核验结论，
//! 按来源与版本固定，不能由 UI 猜测缺失字段。

pub mod claude;
pub mod cline;
pub mod codex;
pub mod dsh;
pub mod framework;
pub mod gemini;
pub mod hermes;
pub mod jsonl;
pub mod kilo;
#[path = "kimi-code/mod.rs"]
pub mod kimi_code;
pub mod kimi_wire;
#[path = "kimi-work/mod.rs"]
pub mod kimi_work;
#[path = "mimo-code/mod.rs"]
pub mod mimo_code;
pub mod omp;
pub mod openclaw;
pub mod opencode;
pub mod opencode_family;
pub mod pi;
pub mod qwen;
pub mod usage_map;
pub mod zcode;
pub mod zoo;
