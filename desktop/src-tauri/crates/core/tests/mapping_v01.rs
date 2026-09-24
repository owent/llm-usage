//! V01：各 provider 缓存包含关系、reasoning 子集、cache TTL 子集映射。
//! 缺失、零、异常负值、溢出必须可区分；矛盾进诊断，不用 max(0,…) 隐藏。

use llm_usage_core::adapters::usage_map::*;
use llm_usage_core::domain::{FieldQuality, TokenUsage, MAX_TOKEN_VALUE};
use llm_usage_core::metrics::{input_total, total_tokens};

#[test]
fn v01_codex_cached_subset_of_input_reasoning_subset_of_output() {
    let m = map_codex(&CodexUsage {
        input_tokens: 1000,
        cached_input_tokens: 800,
        output_tokens: 100,
        reasoning_output_tokens: 40,
        total_tokens: 1100,
        declares_no_cache_creation: true,
    });
    assert_eq!(m.usage.input_uncached, Some(200));
    assert_eq!(m.usage.input_cache_read, Some(800));
    assert_eq!(m.usage.input_cache_write, Some(0));
    assert_eq!(m.usage.input_total, Some(1000));
    assert_eq!(m.usage.output_reasoning, Some(40));
    assert_eq!(
        total_tokens(&m.usage, &m.quality).map(|(v, _)| v),
        Some(1100)
    );
    assert!(m.diagnostics.is_empty());
}

#[test]
fn v01_codex_total_mismatch_is_diagnostic() {
    let m = map_codex(&CodexUsage {
        input_tokens: 1000,
        cached_input_tokens: 0,
        output_tokens: 100,
        reasoning_output_tokens: 0,
        total_tokens: 9999,
        declares_no_cache_creation: true,
    });
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == "source_total_mismatch"));
}

#[test]
fn v01_codex_cached_exceeds_input_not_clamped() {
    let m = map_codex(&CodexUsage {
        input_tokens: 100,
        cached_input_tokens: 800,
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: 110,
        declares_no_cache_creation: true,
    });
    // uncached 派生为负 → 不留 0，uncached 置未知并记诊断。
    assert_eq!(m.usage.input_uncached, None);
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == "negative_derived_field"));
    assert!(m
        .diagnostics
        .iter()
        .any(|d| d.code == "cache_read_exceeds_input_total"));
}

#[test]
fn v01_kimi_wire_four_mutually_exclusive_fields() {
    let m = map_kimi_wire(&KimiWireUsage {
        input_other: 100,
        input_cache_read: 800,
        input_cache_creation: 100,
        output: 100,
    });
    assert_eq!(m.usage.input_total, Some(1000));
    assert_eq!(m.quality.input_total, FieldQuality::Derived);
    assert_eq!(m.usage.total_tokens, Some(1100));
    assert_eq!(m.usage.source_total, None);
    assert_eq!(m.quality.source_total, FieldQuality::Unknown);
}

#[test]
fn v01_zcode_dual_calibers_are_opposite() {
    // 同一逻辑用量（未缓存 200、缓存读 800、输出 100）在两种口径下的原始字段不同。
    let sdk = map_zcode_ai_sdk(&ZcodeAiSdkUsage {
        input_tokens: 1000, // 含缓存读
        cached_input_tokens: Some(800),
        cache_creation_input_tokens: None,
        output_tokens: 100,
        reasoning_tokens: None,
        total_tokens: Some(1100),
    });
    // 缓存创建未知，不能假设为零并补出未缓存输入。
    assert_eq!(sdk.usage.input_uncached, None);
    assert_eq!(sdk.quality.input_uncached, FieldQuality::Unknown);
    assert_eq!(sdk.usage.input_total, Some(1000));
    assert_eq!(sdk.quality.input_total, FieldQuality::Reported);

    let anthropic = map_zcode_anthropic(&ZcodeAnthropicUsage {
        input_tokens: 200, // 不含缓存
        cache_read_input_tokens: Some(800),
        cache_creation_input_tokens: Some(0),
        output_tokens: 100,
    });
    assert_eq!(anthropic.usage.input_uncached, Some(200));
    assert_eq!(anthropic.usage.input_total, Some(1000));
    assert_eq!(anthropic.quality.input_total, FieldQuality::Derived);
    // 两种口径映射后规范化数值一致（质量标记不同是合法的：直报 vs 推导）。
    assert_eq!(
        input_total(&sdk.usage, &sdk.quality).map(|(v, _)| v),
        input_total(&anthropic.usage, &anthropic.quality).map(|(v, _)| v)
    );
    assert_eq!(
        total_tokens(&sdk.usage, &sdk.quality).map(|(v, _)| v),
        Some(1100)
    );
    assert_eq!(
        total_tokens(&anthropic.usage, &anthropic.quality).map(|(v, _)| v),
        Some(1100)
    );
}

#[test]
fn v01_copilot_input_includes_cache_read_and_write() {
    let m = map_copilot(&CopilotUsage {
        input_tokens: 1000,
        cached_input_tokens: 300,
        cache_creation_input_tokens: 200,
        output_tokens: 100,
    });
    assert_eq!(m.usage.input_uncached, Some(500));
    assert_eq!(m.usage.input_cache_read, Some(300));
    assert_eq!(m.usage.input_cache_write, Some(200));
    assert_eq!(m.usage.input_total, Some(1000));
    assert_eq!(
        total_tokens(&m.usage, &m.quality).map(|(v, _)| v),
        Some(1100)
    );
}

#[test]
fn v01_kilo_all_mutually_exclusive_reasoning_not_in_output() {
    let m = map_kilo(&KiloUsage {
        input: 100,
        output: 60,
        reasoning: Some(40),
        cache_read: 800,
        cache_write: 100,
        total: 1100,
    });
    assert_eq!(m.usage.input_total, Some(1000));
    // canonical output_total 并入互斥的 reasoning。
    assert_eq!(m.usage.output_total, Some(100));
    assert_eq!(m.usage.output_reasoning, Some(40));
    assert_eq!(
        total_tokens(&m.usage, &m.quality).map(|(v, _)| v),
        Some(1100)
    );
    assert!(m.diagnostics.is_empty());
}

#[test]
fn v01_missing_zero_negative_overflow_are_distinct() {
    // 缺失：保持 None / unknown，不补零。
    let m = map_kilo(&KiloUsage {
        input: 10,
        output: 5,
        reasoning: None,
        cache_read: 0,
        cache_write: 0,
        total: 15,
    });
    assert_eq!(m.usage.output_reasoning, None);
    assert_eq!(m.quality.output_reasoning, FieldQuality::Unknown);
    // 零值是已知量。
    assert_eq!(m.usage.input_cache_read, Some(0));
    assert_eq!(m.quality.input_cache_read, FieldQuality::Reported);

    // 负值：校验拒绝整条记录。
    let negative = TokenUsage {
        input_total: Some(-1),
        ..TokenUsage::default()
    };
    assert!(negative.validate().is_err());

    // 上限：MAX_TOKEN_VALUE 接受，超限拒绝。
    let at_max = TokenUsage {
        input_total: Some(MAX_TOKEN_VALUE),
        ..TokenUsage::default()
    };
    assert!(at_max.validate().is_ok());
    let over_max = TokenUsage {
        input_total: Some(MAX_TOKEN_VALUE + 1),
        ..TokenUsage::default()
    };
    assert!(over_max.validate().is_err());
}
