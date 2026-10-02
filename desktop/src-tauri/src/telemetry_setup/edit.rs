//! Surgical JSONC/TOML edits. Never deserialize and rewrite a user's whole JSON file.
use jsonc_parser::{ast, common::Ranged, CollectOptions, ParseOptions};
use serde_json::Value;

pub type Change = (Vec<String>, Value);

fn options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

fn validate(value: &ast::Value<'_>) -> Result<(), String> {
    match value {
        ast::Value::Object(obj) => {
            let mut names = std::collections::HashSet::new();
            for prop in &obj.properties {
                if !names.insert(prop.name.as_str()) {
                    return Err("duplicate_keys".into());
                }
                validate(&prop.value)?;
            }
        }
        ast::Value::Array(array) => {
            for value in &array.elements {
                validate(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn json_value(text: &str) -> Result<Value, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // Bound nesting before invoking a recursive parser on a user-controlled file.
    let bytes = text.as_bytes();
    let mut pos = 0;
    let mut depth = 0usize;
    while pos < bytes.len() {
        if bytes[pos] == b'"' {
            pos += 1;
            while pos < bytes.len() && bytes[pos] != b'"' {
                if bytes[pos] == b'\\' {
                    pos += 1;
                }
                pos += 1;
            }
        } else if bytes[pos..].starts_with(b"//") {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
        } else if bytes[pos..].starts_with(b"/*") {
            pos += 2;
            while pos + 1 < bytes.len() && &bytes[pos..pos + 2] != b"*/" {
                pos += 1;
            }
            pos += 1;
        } else if matches!(bytes[pos], b'{' | b'[') {
            depth += 1;
            if depth > 128 {
                return Err("invalid_config".into());
            }
        } else if matches!(bytes[pos], b'}' | b']') {
            depth = depth.saturating_sub(1);
        }
        pos += 1;
    }
    let parsed = jsonc_parser::parse_to_ast(text, &CollectOptions::default(), &options())
        .map_err(|_| "invalid_config".to_string())?;
    let value = parsed.value.ok_or("invalid_config")?;
    if value.as_object().is_none() {
        return Err("invalid_config".into());
    }
    validate(&value)?;
    jsonc_parser::parse_to_serde_value(text, &options()).map_err(|_| "invalid_config".into())
}

/// First significant byte after a value, skipping comments. Needed for trailing commas.
fn significant(text: &str, mut at: usize) -> Option<(usize, u8)> {
    let bytes = text.as_bytes();
    while at < bytes.len() {
        if bytes[at].is_ascii_whitespace() {
            at += 1;
        } else if bytes[at..].starts_with(b"//") {
            at += 2;
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
        } else if bytes[at..].starts_with(b"/*") {
            at += 2;
            while at + 1 < bytes.len() && &bytes[at..at + 2] != b"*/" {
                at += 1;
            }
            at += 2;
        } else {
            return Some((at, bytes[at]));
        }
    }
    None
}

fn json_edit(text: &str, path: &[String], value: &Value) -> Result<String, String> {
    let parsed = jsonc_parser::parse_to_ast(text, &CollectOptions::default(), &options())
        .map_err(|_| "invalid_config".to_string())?;
    let root = parsed.value.ok_or("invalid_config")?;
    let mut object = root.as_object().ok_or("invalid_config")?;
    for (i, key) in path.iter().enumerate() {
        if let Some(prop) = object.properties.iter().find(|p| p.name.as_str() == key) {
            if i + 1 == path.len() {
                let range = prop.value.range();
                let mut output = text.to_owned();
                output.replace_range(range.start..range.end, &value.to_string());
                return Ok(output);
            }
            object = prop.value.as_object().ok_or("invalid_config")?;
        } else {
            let mut nested = value.clone();
            for key in path[i + 1..].iter().rev() {
                nested = serde_json::json!({key: nested});
            }
            let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let insert = format!("  {}: {nested}{nl}", serde_json::to_string(key).unwrap());
            let mut output = text.to_owned();
            let close = object.range.end - 1;
            // Reuse the closing brace's newline instead of adding another one
            // on every inserted property. Existing comments/blank lines stay.
            let line_start = text[..close].rfind('\n').map_or(close, |p| p + 1);
            if line_start < close && text[line_start..close].trim().is_empty() {
                output.insert_str(line_start, &insert);
            } else if text[..close].ends_with('\n') {
                output.insert_str(close, &insert);
            } else {
                output.insert_str(close, &format!("{nl}{insert}"));
            }
            if let Some(last) = object.properties.last() {
                let end = last.value.range().end;
                if significant(text, end).map(|(_, b)| b) != Some(b',') {
                    output.insert(end, ',');
                }
            }
            return Ok(output);
        }
    }
    Err("invalid_config".into())
}

pub fn merge_json(text: &str, changes: &[Change]) -> Result<String, String> {
    let bom = text.starts_with('\u{feff}');
    let mut output = text.strip_prefix('\u{feff}').unwrap_or(text).to_owned();
    let mut value = json_value(&output)?;
    for (path, next) in changes {
        if lookup(&value, path) != Some(next) {
            output = json_edit(&output, path, next)?;
            value = json_value(&output)?;
        }
    }
    Ok(if bom {
        format!("\u{feff}{output}")
    } else {
        output
    })
}

pub fn lookup<'a>(mut value: &'a Value, path: &[String]) -> Option<&'a Value> {
    for key in path {
        value = value.get(key)?;
    }
    Some(value)
}

fn remove_json(text: &str, path: &[String]) -> Result<String, String> {
    let parsed = jsonc_parser::parse_to_ast(text, &CollectOptions::default(), &options())
        .map_err(|_| "invalid_config")?;
    let root = parsed.value.ok_or("invalid_config")?;
    let mut object = root.as_object().ok_or("invalid_config")?;
    for key in &path[..path.len() - 1] {
        object = object
            .properties
            .iter()
            .find(|p| p.name.as_str() == key)
            .and_then(|p| p.value.as_object())
            .ok_or("invalid_config")?;
    }
    let index = object
        .properties
        .iter()
        .position(|p| Some(p.name.as_str()) == path.last().map(String::as_str))
        .ok_or("invalid_config")?;
    let prop = &object.properties[index];
    let range = prop.range();
    let comma = significant(text, prop.value.range().end)
        .filter(|(_, b)| *b == b',')
        .map(|(pos, _)| pos)
        .or_else(|| {
            if index > 0 {
                significant(text, object.properties[index - 1].value.range().end)
                    .filter(|(_, b)| *b == b',')
                    .map(|(pos, _)| pos)
            } else {
                None
            }
        });
    let mut edits = vec![(range.start, range.end)];
    if let Some(pos) = comma {
        edits.push((pos, pos + 1));
    }
    edits.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    let mut output = text.to_owned();
    for (start, end) in edits {
        output.replace_range(start..end, "");
    }
    Ok(output)
}

/// Revert owned values only. Later user edits to other keys are preserved, and edited
/// owned values are reported as conflicts rather than being overwritten.
pub fn restore_json(
    text: &str,
    before: &Value,
    changes: &[Change],
) -> Result<(String, Vec<String>), String> {
    let bom = text.starts_with('\u{feff}');
    let mut output = text.strip_prefix('\u{feff}').unwrap_or(text).to_owned();
    let mut conflicts = Vec::new();
    for (path, after) in changes {
        let current = json_value(&output)?;
        if lookup(&current, path) != Some(after) {
            conflicts.push(path.join("."));
            continue;
        }
        output = if let Some(old) = lookup(before, path) {
            merge_json(&output, &[(path.clone(), old.clone())])?
        } else {
            remove_json(&output, path)?
        };
        json_value(&output)?;
    }
    Ok((
        if bom {
            format!("\u{feff}{output}")
        } else {
            output
        },
        conflicts,
    ))
}

pub fn restore_toml(
    text: &str,
    before: &Value,
    changes: &[Change],
) -> Result<(String, Vec<String>), String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| "invalid_config")?;
    let current = toml_value(text)?;
    let mut conflicts = Vec::new();
    for (path, after) in changes {
        if lookup(&current, path) != Some(after) {
            conflicts.push(path.join("."));
            continue;
        }
        let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
        for key in &path[..path.len() - 1] {
            table = table
                .get_mut(key)
                .and_then(toml_edit::Item::as_table_like_mut)
                .ok_or("invalid_config")?;
        }
        let key = path.last().ok_or("invalid_config")?;
        if let Some(old) = lookup(before, path) {
            table.insert(key, to_item(old)?);
        } else {
            table.remove(key);
        }
    }
    let output = doc.to_string();
    toml_value(&output)?;
    Ok((output, conflicts))
}

pub fn toml_value(text: &str) -> Result<Value, String> {
    toml_edit::de::from_str(text).map_err(|_| "invalid_config".into())
}

fn to_item(value: &Value) -> Result<toml_edit::Item, String> {
    match value {
        Value::Bool(v) => Ok(toml_edit::value(*v)),
        Value::String(v) => Ok(toml_edit::value(v.clone())),
        Value::Object(map) => {
            let mut table = toml_edit::Table::new();
            for (k, v) in map {
                table.insert(k, to_item(v)?);
            }
            Ok(toml_edit::Item::Table(table))
        }
        _ => Err("invalid_config".into()),
    }
}

pub fn merge_toml(text: &str, changes: &[Change]) -> Result<String, String> {
    let original = toml_value(text)?;
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| "invalid_config")?;
    for (path, next) in changes {
        if lookup(&original, path) == Some(next) {
            continue;
        }
        let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
        for key in &path[..path.len() - 1] {
            if !table.contains_key(key) {
                table.insert(key, toml_edit::Item::Table(toml_edit::Table::new()));
            }
            table = table
                .get_mut(key)
                .and_then(toml_edit::Item::as_table_like_mut)
                .ok_or("invalid_config")?;
        }
        table.insert(path.last().ok_or("invalid_config")?, to_item(next)?);
    }
    let output = doc.to_string();
    toml_value(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn change(path: &[&str], value: Value) -> Change {
        (path.iter().map(|s| s.to_string()).collect(), value)
    }
    #[test]
    fn jsonc_preserves_bom_comments_unicode_credentials_crlf_and_unrelated_bytes() {
        let text = "\u{feff}{\r\n// 保留\r\n\"secret\": \"opaque\",\r\n\"telemetry\": {\"enabled\":false /* note */},\r\n}\r\n";
        let changes = [
            change(&["telemetry", "enabled"], Value::Bool(true)),
            change(
                &["telemetry", "outfile"],
                Value::String("C:/a'b/遥测".into()),
            ),
        ];
        let output = merge_json(text, &changes).unwrap();
        assert!(output.starts_with('\u{feff}'));
        assert!(output.contains("// 保留\r\n\"secret\": \"opaque\","));
        assert!(output.contains("true, /* note */"));
        assert!(output.contains("C:/a'b/遥测"));
        assert_eq!(merge_json(&output, &changes).unwrap(), output);
    }
    #[test]
    fn jsonc_missing_nested_keys_and_comment_commas_are_safe() {
        for text in [
            "{}",
            "{/* empty */}",
            "{\"x\":1 // comma ,\n}",
            "{\"x\":1, /* tail */}",
        ] {
            let output =
                merge_json(text, &[change(&["env", "FLAG"], Value::String("1".into()))]).unwrap();
            assert_eq!(json_value(&output).unwrap()["env"]["FLAG"], "1");
        }
    }
    #[test]
    fn rejects_invalid_root_duplicate_escaped_keys_and_wrong_nested_shape() {
        for text in [
            "[]",
            "{\"a\":1,\"\\u0061\":2}",
            "{\"env\":{\"x\":1,\"x\":2}}",
            "{unquoted:true}",
            "{\"x\":1 \"y\":2}",
        ] {
            assert!(json_value(text).is_err(), "{text}");
        }
        assert!(merge_json(
            "{\"env\":false}",
            &[change(&["env", "FLAG"], Value::Bool(true))]
        )
        .is_err());
    }
    #[test]
    fn deeply_nested_configs_are_rejected_without_recursing_but_braces_in_comments_are_text() {
        let deep = format!("{{\"x\":{}0{}}}", "[".repeat(512), "]".repeat(512));
        assert!(json_value(&deep).is_err());
        let shallow = format!("{{/* {} */\"x\":\"{}\"}}", "[".repeat(512), "[".repeat(512));
        assert!(json_value(&shallow).is_ok());
    }
    #[test]
    fn toml_preserves_other_tables_and_comments_and_is_idempotent() {
        let text = "# user\nmodel = 'my-model'\n[other]\nsecret = 'opaque' # keep\n[otel]\nexporter = 'none'\n";
        let changes = [
            change(&["otel", "log_user_prompt"], Value::Bool(false)),
            change(
                &["otel", "exporter"],
                serde_json::json!({"otlp-http":{"endpoint":"http://127.0.0.1:4318/v1/logs","protocol":"json"}}),
            ),
        ];
        let output = merge_toml(text, &changes).unwrap();
        assert!(output.contains("secret = 'opaque' # keep"));
        assert_eq!(
            toml_value(&output).unwrap()["otel"]["exporter"]["otlp-http"]["protocol"],
            "json"
        );
        assert_eq!(merge_toml(&output, &changes).unwrap(), output);
        assert!(merge_toml("otel = false", &changes).is_err());
    }
    #[test]
    fn undo_preserves_later_user_changes_and_reports_edited_owned_keys() {
        let before = json!({"telemetry":{"enabled":false},"editor":12});
        let changes = [
            change(&["telemetry", "enabled"], json!(true)),
            change(&["telemetry", "outfile"], json!("owned")),
        ];
        let current="{/* keep */\"telemetry\":{\"enabled\":true,\"outfile\":\"user-changed\",\"other\":9},\"editor\":20}";
        let (output, conflicts) = restore_json(current, &before, &changes).unwrap();
        let value = json_value(&output).unwrap();
        assert_eq!(value["editor"], 20);
        assert_eq!(value["telemetry"]["other"], 9);
        assert_eq!(value["telemetry"]["enabled"], false);
        assert_eq!(value["telemetry"]["outfile"], "user-changed");
        assert!(output.contains("/* keep */"));
        assert_eq!(conflicts, ["telemetry.outfile"]);
    }
    #[test]
    fn undo_removes_inserted_properties_with_first_last_and_trailing_commas() {
        for text in [
            "{\"owned\":true,\"other\":1}",
            "{\"other\":1,\"owned\":true}",
            "{\"owned\":true,}",
            "{\"owned\":true /* , */}",
        ] {
            let (output, conflicts) =
                restore_json(text, &json!({}), &[change(&["owned"], json!(true))]).unwrap();
            assert!(conflicts.is_empty());
            let value = json_value(&output).unwrap();
            assert!(value.get("owned").is_none());
            if text.contains("other") {
                assert_eq!(value["other"], 1);
            }
        }
    }
    #[test]
    fn toml_undo_retains_later_unrelated_table_edits() {
        let before = json!({"otel":{"exporter":"none"}});
        let changes = [change(&["otel", "log_user_prompt"], json!(false))];
        let (output, conflicts) = restore_toml(
            "model = 'changed'\n[otel]\nlog_user_prompt=false\nexporter='none'\n",
            &before,
            &changes,
        )
        .unwrap();
        assert!(conflicts.is_empty());
        assert_eq!(toml_value(&output).unwrap()["model"], "changed");
        assert!(toml_value(&output).unwrap()["otel"]
            .get("log_user_prompt")
            .is_none());
    }
}
