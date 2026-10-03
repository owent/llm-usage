//! Kilo 版本注册表：`session.version` → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 版本标记是 kilo.db `session.version` 列（写消息时的 CLI 版本，逐会话固定）；
//! 一个库可混存多版本会话，探测取数值最大者作为该库的格式标记。
//!
//! 已验证版本须有真实脱敏 fixture 与期望值证据：
//!
//! - 7.4.8（session-7.4.8-edges：错误消息 tokens 全零、缺 total、双模型切换）；
//! - 7.4.9（session-7.4.9-family：父子会话家族，5 会话 51 调用全链路期望）；
//! - 7.8.1（session-7.8.1-k3：本机实读 7.8.1 会话脱敏，34 次 k3-256k 调用，
//!   顶层 modelID/providerID、tokens 五字段，单会话对账 matched）。
//!
//! 三者 message.data.tokens 载体同形，共用 `message_tokens_v1`。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`，按映射分派；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器，
//!   通过校验的数据带兼容标记入库（本机实读库观测 7.3.42–7.7.12，
//!   仅 7.4.8/7.4.9 有 fixture 证据，其余均走 LatestFallback）；
//! - kilo 无已证实不兼容的版本；schema 偏离在探测层 fail closed，
//!   结构不兼容在扫描层按 V30 判定并保留旧结果。

pub mod message_tokens_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "message_tokens_v1";

/// 已验证支持的 session.version → 格式实现。
/// 每个版本都有真实脱敏 fixture 与人工核算期望；同形版本共用实现，
/// 分派仍逐版本登记（注册表扩展只增加条目，不删除历史实现）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("7.4.8", "message_tokens_v1"),
    ("7.4.9", "message_tokens_v1"),
    ("7.8.1", "message_tokens_v1"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始版本选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
pub fn select(found: Option<&str>) -> Selection {
    match found {
        Some(version) => {
            let known = VERIFIED_VERSION_IMPLS
                .iter()
                .find(|(v, _)| *v == version)
                .map(|(_, impl_id)| *impl_id);
            match known {
                Some(impl_id) => Selection {
                    impl_id,
                    basis: crate::domain::VersionBasis::KnownVersion,
                },
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // 版本字段缺失但 Agent 身份/输入类型已确认：默认回退最新实现。
        None => Selection {
            impl_id: LATEST_IMPL_ID,
            basis: crate::domain::VersionBasis::LatestFallback,
        },
    }
}

/// 数值感知的版本比较（"7.4.10" > "7.4.9"，字符串序会误判）。
/// 非数值段按 i64::MIN 折叠后逐段比较；完全无法解析时退回字典序。
pub(crate) fn version_max<'a>(a: &'a str, b: &'a str) -> &'a str {
    let parse = |v: &str| -> Vec<i64> {
        v.split(['.', '-', '+'])
            .map(|part| part.parse::<i64>().unwrap_or(i64::MIN))
            .collect()
    };
    let (na, nb) = (parse(a), parse(b));
    if na != nb {
        return if na > nb { a } else { b };
    }
    if a >= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_versions_dispatch_known() {
        for v in ["7.4.8", "7.4.9", "7.8.1"] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "message_tokens_v1",
                    basis: VersionBasis::KnownVersion
                }
            );
        }
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        // 本机实读库观测到但无 fixture 证据的版本：latest_fallback，不拒绝。
        for v in ["7.3.42", "7.4.20", "7.7.12", "9.0.0"] {
            assert_eq!(select(Some(v)).basis, VersionBasis::LatestFallback);
        }
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }

    #[test]
    fn version_max_is_numeric_aware() {
        assert_eq!(version_max("7.4.9", "7.4.10"), "7.4.10");
        assert_eq!(version_max("7.4.10", "7.4.9"), "7.4.10");
        assert_eq!(version_max("7.10.0", "7.9.9"), "7.10.0");
        assert_eq!(version_max("7.4.8", "7.4.8"), "7.4.8");
    }
}
