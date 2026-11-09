//! OTLP/HTTP JSON decoding: one flat record per log record and metric data point.

use gitprompt_capture::otlp::records;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn attr(key: &str, value: Value) -> Value {
    json!({"key": key, "value": value})
}

fn logs(resource: Vec<Value>, records: Vec<Value>) -> Value {
    json!({"resourceLogs": [{"resource": {"attributes": resource}, "scopeLogs": [{"logRecords": records}]}]})
}

fn metrics(resource: Vec<Value>, metrics: Vec<Value>) -> Value {
    json!({"resourceMetrics": [{"resource": {"attributes": resource}, "scopeMetrics": [{"metrics": metrics}]}]})
}

#[test]
fn a_log_record_carries_resource_attributes_its_own_attributes_body_and_time() {
    let body = logs(
        vec![attr("service.name", json!({"stringValue": "claude-code"}))],
        vec![json!({
            "timeUnixNano": "1762500000000000000",
            "body": {"stringValue": "claude_code.api_request"},
            "attributes": [attr("event.name", json!({"stringValue": "api_request"})), attr("prompt.id", json!({"stringValue": "p1"}))]
        })],
    );
    assert_eq!(
        records(&body),
        vec![json!({
            "service.name": "claude-code",
            "event.name": "api_request",
            "prompt.id": "p1",
            "body": "claude_code.api_request",
            "time_unix_nano": "1762500000000000000"
        })]
    );
}

#[test]
fn int_values_sent_as_strings_are_decoded_as_integers() {
    let body = logs(vec![], vec![json!({"attributes": [attr("input_tokens", json!({"intValue": "120"}))]})]);
    assert_eq!(records(&body)[0]["input_tokens"], json!(120));
}

#[test]
fn double_bool_bytes_kvlist_and_array_values_keep_their_shape() {
    let kvlist = json!({"kvlistValue": {"values": [attr("a", json!({"intValue": "1"})), attr("b", json!({"stringValue": "x"}))]}});
    let array = json!({"arrayValue": {"values": [{"stringValue": "x"}, {"intValue": "2"}, {"boolValue": false}]}});
    let body = logs(
        vec![],
        vec![json!({"attributes": [
            attr("cost_usd", json!({"doubleValue": 0.002})),
            attr("success", json!({"boolValue": true})),
            attr("blob", json!({"bytesValue": "AAEC"})),
            attr("nested", kvlist),
            attr("list", array),
        ]})],
    );
    let record = &records(&body)[0];
    assert_eq!(record["cost_usd"], json!(0.002));
    assert_eq!(record["success"], json!(true));
    assert_eq!(record["blob"], json!("AAEC"));
    assert_eq!(record["nested"], json!({"a": 1, "b": "x"}));
    assert_eq!(record["list"], json!(["x", 2, false]));
}

#[test]
fn an_empty_kvlist_or_array_value_is_an_empty_object_or_list() {
    let body = logs(vec![], vec![json!({"attributes": [attr("o", json!({"kvlistValue": {}})), attr("l", json!({"arrayValue": {}}))]})]);
    let record = &records(&body)[0];
    assert_eq!((&record["o"], &record["l"]), (&json!({}), &json!([])));
}

#[test]
fn a_record_attribute_wins_over_the_resource_attribute_of_the_same_key() {
    let body = logs(
        vec![attr("session.id", json!({"stringValue": "from-resource"})), attr("os.type", json!({"stringValue": "darwin"}))],
        vec![json!({"attributes": [attr("session.id", json!({"stringValue": "from-record"}))]})],
    );
    let record = &records(&body)[0];
    assert_eq!((&record["session.id"], &record["os.type"]), (&json!("from-record"), &json!("darwin")));
}

#[test]
fn a_record_without_body_time_or_value_gets_nulls() {
    let body = logs(vec![], vec![json!({"attributes": [attr("empty", json!({}))]})]);
    assert_eq!(records(&body), vec![json!({"empty": null, "body": null, "time_unix_nano": null})]);
}

#[test]
fn a_sum_data_point_becomes_a_record_named_after_its_metric() {
    let body = metrics(
        vec![attr("service.name", json!({"stringValue": "claude-code"}))],
        vec![json!({
            "name": "claude_code.cost.usage",
            "unit": "USD",
            "sum": {"dataPoints": [{"asDouble": 0.25, "timeUnixNano": "17", "attributes": [attr("model", json!({"stringValue": "m"}))]}]}
        })],
    );
    assert_eq!(
        records(&body),
        vec![json!({
            "service.name": "claude-code",
            "model": "m",
            "event.name": "claude_code.cost.usage",
            "unit": "USD",
            "value": 0.25,
            "time_unix_nano": "17"
        })]
    );
}

#[test]
fn an_int_data_point_sent_as_a_string_becomes_a_float_value() {
    let body = metrics(vec![], vec![json!({"name": "claude_code.session.count", "gauge": {"dataPoints": [{"asInt": "3"}]}})]);
    let record = &records(&body)[0];
    assert_eq!(record["value"], json!(3.0));
    assert!(record["value"].is_f64(), "value is a float: {}", record["value"]);
}

#[test]
fn a_histogram_data_point_is_reported_by_its_sum() {
    let body = metrics(vec![], vec![json!({"name": "latency", "unit": "ms", "histogram": {"dataPoints": [{"sum": 42, "count": "3"}]}})]);
    assert_eq!(records(&body)[0]["value"], json!(42.0));
}

#[test]
fn a_data_point_without_a_value_or_unit_gets_nulls() {
    let body = metrics(vec![], vec![json!({"name": "m", "sum": {"dataPoints": [{}]}})]);
    let record = &records(&body)[0];
    assert_eq!((&record["value"], &record["unit"]), (&Value::Null, &Value::Null));
}

#[test]
fn a_metric_of_an_unknown_kind_yields_no_record() {
    let body = metrics(vec![], vec![json!({"name": "m", "exponentialHistogram": {"dataPoints": [{"sum": 1}]}})]);
    assert_eq!(records(&body), Vec::<Value>::new());
}

#[test]
fn logs_come_before_metrics_and_each_resource_merges_only_its_own_attributes() {
    let body = json!({
        "resourceMetrics": [{"resource": {"attributes": [attr("r", json!({"stringValue": "m"}))]},
                              "scopeMetrics": [{"metrics": [{"name": "points", "sum": {"dataPoints": [{"asInt": "1"}]}}]}]}],
        "resourceLogs": [
            {"resource": {"attributes": [attr("r", json!({"stringValue": "first"}))]},
             "scopeLogs": [{"logRecords": [{"attributes": [attr("n", json!({"intValue": "1"}))]}]}]},
            {"resource": {"attributes": []},
             "scopeLogs": [{"logRecords": [{"attributes": [attr("n", json!({"intValue": "2"}))]}]},
                           {"logRecords": [{"attributes": [attr("n", json!({"intValue": "3"}))]}]}]}
        ]
    });
    let out = records(&body);
    let order: Vec<(&Value, &Value)> =
        out.iter().map(|r| (r.get("n").unwrap_or(&Value::Null), r.get("r").unwrap_or(&Value::Null))).collect();
    assert_eq!(
        order,
        vec![(&json!(1), &json!("first")), (&json!(2), &Value::Null), (&json!(3), &Value::Null), (&Value::Null, &json!("m"))]
    );
}

#[test]
fn an_export_without_logs_or_metrics_yields_nothing() {
    assert_eq!(records(&json!({})), Vec::<Value>::new());
    assert_eq!(records(&json!({"resourceLogs": [{"scopeLogs": [{}]}]})), Vec::<Value>::new());
}
