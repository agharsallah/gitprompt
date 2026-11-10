//! Flatten OTLP/HTTP JSON export requests (logs and metrics) into one plain JSON object per record.
//!
//! Resource attributes (app version, terminal, OS...) are merged into every record, so each stored
//! event is self-describing; a record's own attribute wins over a resource attribute of the same key.
//! Spec: <https://opentelemetry.io/docs/specs/otlp/#json-protobuf-encoding>. Port of `capture/otlp.py`.
//!
//! Values (`AnyValue`): `stringValue`, `boolValue`, `doubleValue`, `bytesValue` as is; `intValue`
//! (JSON-encoded as a string) as an integer; `kvlistValue` as an object; `arrayValue` as an array;
//! an empty value as `null`.
//!
//! A log record becomes `resource attrs ∪ record attrs ∪ {body, time_unix_nano}`.
//! A metric data point (of a `sum`, `gauge` or `histogram`) becomes
//! `resource attrs ∪ point attrs ∪ {"event.name": <metric name>, unit, value, time_unix_nano}`, where
//! `value` is `asDouble`, else `asInt`, else the histogram `sum`, as a float (`null` when none).

use gitprompt_core::{Json, JsonMap};

/// The metric kinds whose data points become records; any other kind is skipped.
const METRIC_KINDS: [&str; 3] = ["sum", "gauge", "histogram"];

/// Every log record and metric data point of an export request, logs first, in document order.
/// Missing sections are empty, never an error.
pub fn records(body: &Json) -> Vec<Json> {
    let mut out = Vec::new();
    for resource in items(body, "resourceLogs") {
        let base = resource_attrs(resource);
        for log in items(resource, "scopeLogs").iter().flat_map(|scope| items(scope, "logRecords")) {
            let mut record = with_attrs(&base, log);
            record.insert("body".into(), log.get("body").map_or(Json::Null, value));
            record.insert("time_unix_nano".into(), field(log, "timeUnixNano"));
            out.push(Json::Object(record));
        }
    }
    for resource in items(body, "resourceMetrics") {
        let base = resource_attrs(resource);
        for metric in items(resource, "scopeMetrics").iter().flat_map(|scope| items(scope, "metrics")) {
            let Some(data) = METRIC_KINDS.iter().find_map(|kind| metric.get(*kind)) else { continue };
            for point in items(data, "dataPoints") {
                let mut record = with_attrs(&base, point);
                record.insert("event.name".into(), field(metric, "name"));
                record.insert("unit".into(), field(metric, "unit"));
                record.insert("value".into(), point_value(point));
                record.insert("time_unix_nano".into(), field(point, "timeUnixNano"));
                out.push(Json::Object(record));
            }
        }
    }
    out
}

/// The array at `key` of an object; empty when missing or not an array.
fn items<'a>(node: &'a Json, key: &str) -> &'a [Json] {
    node.get(key).and_then(Json::as_array).map_or(&[], Vec::as_slice)
}

/// The value at `key`, `null` when missing.
fn field(node: &Json, key: &str) -> Json {
    node.get(key).cloned().unwrap_or(Json::Null)
}

fn resource_attrs(resource: &Json) -> JsonMap {
    resource.get("resource").map(|r| attrs(r.get("attributes"))).unwrap_or_default()
}

/// `base` with the node's own `attributes` on top.
fn with_attrs(base: &JsonMap, node: &Json) -> JsonMap {
    let mut record = base.clone();
    record.extend(attrs(node.get("attributes")));
    record
}

/// A `KeyValue` list as an object; entries without a string key are skipped.
fn attrs(list: Option<&Json>) -> JsonMap {
    let Some(Json::Array(list)) = list else { return JsonMap::new() };
    list.iter()
        .filter_map(|kv| {
            let key = kv.get("key")?.as_str()?;
            Some((key.to_owned(), kv.get("value").map_or(Json::Null, value)))
        })
        .collect()
}

/// An `AnyValue` as plain JSON.
fn value(any: &Json) -> Json {
    let Some(map) = any.as_object() else { return Json::Null };
    if let Some(kvlist) = map.get("kvlistValue") {
        return Json::Object(attrs(kvlist.get("values")));
    }
    if let Some(array) = map.get("arrayValue") {
        return Json::Array(items(array, "values").iter().map(value).collect());
    }
    if let Some(int) = map.get("intValue") {
        return integer(int);
    }
    // stringValue, boolValue, doubleValue, bytesValue: the one value it holds.
    map.values().next().cloned().unwrap_or(Json::Null)
}

/// An `intValue`, JSON-encoded as a string (64-bit integers don't fit a JSON number everywhere).
/// A string that isn't an integer is kept as sent.
fn integer(int: &Json) -> Json {
    match int.as_str() {
        Some(text) => text.parse::<i64>().map(Json::from).or_else(|_| text.parse::<u64>().map(Json::from)).unwrap_or_else(|_| int.clone()),
        None => int.clone(),
    }
}

/// `asDouble`, else `asInt`, else the histogram `sum`, as a float; `null` when none is a number.
fn point_value(point: &Json) -> Json {
    let raw = ["asDouble", "asInt", "sum"].iter().find_map(|key| point.get(*key));
    let number = match raw {
        Some(Json::Number(n)) => n.as_f64(),
        Some(Json::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    };
    number.and_then(serde_json::Number::from_f64).map_or(Json::Null, Json::Number)
}
