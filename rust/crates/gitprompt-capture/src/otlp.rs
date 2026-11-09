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

use gitprompt_core::Json;

/// Every log record and metric data point of an export request, logs first, in document order.
/// Missing sections are empty, never an error.
pub fn records(_body: &Json) -> Vec<Json> {
    todo!()
}
