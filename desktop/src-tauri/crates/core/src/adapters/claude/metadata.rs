use serde_json::Value;

// Native 2.1.312 Windows samples and the distributed writer distinguish request
// replay/state records from assistant usage. Never count their cumulative copies.
pub(crate) fn validate(value: &Value) -> Option<Result<(), &'static str>> {
    let kind = value.get("type")?.as_str()?;
    if ![
        "atis-latch",
        "tools-used",
        "api-request-shape",
        "api-request-blob",
        "api-request",
        "cost-state",
        "file-history-snapshot",
        "progress",
    ]
    .contains(&kind)
    {
        return None;
    }
    let text = |key: &str| value.get(key).is_some_and(Value::is_string);
    let object = |key: &str| value.get(key).is_some_and(Value::is_object);
    let array = |key: &str| value.get(key).is_some_and(Value::is_array);
    let check = |valid: bool, field: &'static str| -> Result<(), &'static str> {
        if valid {
            Ok(())
        } else {
            Err(field)
        }
    };
    Some((|| -> Result<(), &'static str> {
        match kind {
            "atis-latch" => {
                check(text("sessionId"), "sessionId")?;
                check(text("atis"), "atis")?;
            }
            "tools-used" => {
                check(text("sessionId"), "sessionId")?;
                check(
                    value
                        .get("tools")
                        .and_then(Value::as_array)
                        .is_some_and(|tools| tools.iter().all(Value::is_string)),
                    "tools",
                )?;
            }
            "api-request-shape" => {
                check(text("sessionId"), "sessionId")?;
                check(text("shapeHash"), "shapeHash")?;
                check(text("timestamp"), "timestamp")?;
                check(text("version"), "version")?;
                check(object("shape"), "shape")?;
            }
            "api-request-blob" => {
                check(text("sessionId"), "sessionId")?;
                check(text("hash"), "hash")?;
                check(object("message"), "message")?;
            }
            "api-request" => {
                check(text("sessionId"), "sessionId")?;
                check(text("id"), "id")?;
                check(text("timestamp"), "timestamp")?;
                check(text("version"), "version")?;
                check(text("querySource"), "querySource")?;
                check(text("shapeHash"), "shapeHash")?;
                check(object("params"), "params")?;
                check(value.get("keep").and_then(Value::as_u64).is_some(), "keep")?;
                check(array("tail"), "tail")?;
                check(array("cacheControl"), "cacheControl")?;
                check(
                    text("requestDigest") || text("cutRequestDigest"),
                    "requestDigest",
                )?;
            }
            "cost-state" => {
                check(text("sessionId"), "sessionId")?;
                check(object("modelUsage"), "modelUsage")?;
                check(
                    value
                        .get("hasUnknownModelCost")
                        .is_some_and(Value::is_boolean),
                    "hasUnknownModelCost",
                )?;
                for key in [
                    "totalCostUSD",
                    "totalAPIDuration",
                    "totalAPIDurationWithoutRetries",
                    "totalToolDuration",
                    "totalLinesAdded",
                    "totalLinesRemoved",
                    "totalDuration",
                    "startTime",
                ] {
                    check(
                        value
                            .get(key)
                            .and_then(Value::as_f64)
                            .is_some_and(|n| n.is_finite() && n >= 0.0),
                        key,
                    )?;
                }
            }
            "file-history-snapshot" => {
                check(text("messageId"), "messageId")?;
                check(object("snapshot"), "snapshot")?;
                check(
                    value.get("isSnapshotUpdate").is_some_and(Value::is_boolean),
                    "isSnapshotUpdate",
                )?;
            }
            "progress" => {
                check(object("data"), "data")?;
                check(text("toolUseID"), "toolUseID")?;
            }
            _ => unreachable!(),
        }
        check(value.get("usage").is_none(), "usage")?;
        check(value.pointer("/message/usage").is_none(), "message.usage")?;
        Ok(())
    })())
}
