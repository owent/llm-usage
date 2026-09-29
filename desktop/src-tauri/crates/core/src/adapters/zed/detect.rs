//! Zed 探测：threads.db 的表/列指纹（官方源码 bd74733 建表 + 迁移列集）。
//!
//! 本机核验（2026-09-29 只读 `%LOCALAPPDATA%/Zed/threads/threads.db`）：
//! threads 表实际存在且列集与官方迁移 SQL 一致（0 行，schema 级证据）。
//!
//! 合同（V17 fail closed）：
//! - 非 SQLite/无 threads 表/缺必需列 ⇒ 未知格式，不交给猜测逻辑；
//! - threads 表存在（空库）⇒ Supported（Pending 场景由框架空文件路径处理）；
//! - 无产品版本可读：格式锚点是文档级 zed-threads-db-1（官方源码口径）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const ZED_FORMAT: &str = "zed-threads-db";

/// 探测一个 threads.db：threads 表 + 必需列指纹。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let conn = match common::open_readonly(path) {
        Ok(conn) => conn,
        Err(err) if common::is_busy_like(&err) => {
            // busy/锁：本轮无法判定，Pending 下轮重探（不误报未知格式）。
            return Ok(DetectOutcome::Pending);
        }
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable SQLite database".to_string(),
            });
        }
    };
    let columns: Vec<String> = match conn.prepare("PRAGMA table_info(threads)") {
        Ok(mut stmt) => {
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Ok(Some(row)) = rows.next() {
                if let Ok(name) = row.get::<_, String>(1) {
                    out.push(name);
                }
            }
            out
        }
        Err(_) => Vec::new(),
    };
    if columns.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "no threads table; not a Zed threads database".to_string(),
        });
    }
    let missing: Vec<&str> = common::REQUIRED_THREADS_COLUMNS
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("threads table missing required columns: {missing:?}"),
        });
    }
    // created_at 是迁移列（官方 db.rs:460-483）：缺失按旧库处理（扫描层降级）。
    Ok(DetectOutcome::Supported {
        format: ZED_FORMAT.to_string(),
        format_version: Some(versions::ZED_FORMAT_VERSION.to_string()),
        basis: VersionBasis::KnownVersion,
    })
}
