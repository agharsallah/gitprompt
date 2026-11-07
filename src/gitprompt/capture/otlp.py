"""Flatten OTLP/HTTP JSON export requests (logs and metrics) into one plain dict per record.

Resource attributes (app version, terminal, OS...) are merged into every record, so each stored
event is self-describing. Spec: https://opentelemetry.io/docs/specs/otlp/#json-protobuf-encoding
"""

from collections.abc import Iterator
from typing import Any


def _value(v: dict) -> Any:
    if "kvlistValue" in v:
        return _attrs(v["kvlistValue"].get("values"))
    if "arrayValue" in v:
        return [_value(x) for x in v["arrayValue"].get("values", [])]
    if "intValue" in v:
        return int(v["intValue"])  # JSON-encoded as a string
    return next(iter(v.values()), None)  # stringValue, boolValue, doubleValue, bytesValue


def _attrs(items: list[dict] | None) -> dict[str, Any]:
    return {a["key"]: _value(a.get("value", {})) for a in items or []}


def records(body: dict) -> Iterator[dict[str, Any]]:
    for res in body.get("resourceLogs", []):
        resource = _attrs(res.get("resource", {}).get("attributes"))
        for scope in res.get("scopeLogs", []):
            for log in scope.get("logRecords", []):
                yield (
                    resource
                    | _attrs(log.get("attributes"))
                    | {"body": _value(log.get("body", {})), "time_unix_nano": log.get("timeUnixNano")}
                )
    for res in body.get("resourceMetrics", []):
        resource = _attrs(res.get("resource", {}).get("attributes"))
        for scope in res.get("scopeMetrics", []):
            for metric in scope.get("metrics", []):
                data = next((metric[k] for k in ("sum", "gauge", "histogram") if k in metric), {})
                for point in data.get("dataPoints", []):
                    value = point.get("asDouble", point.get("asInt", point.get("sum")))
                    yield (
                        resource
                        | _attrs(point.get("attributes"))
                        | {
                            "event.name": metric["name"],
                            "unit": metric.get("unit"),
                            "value": None if value is None else float(value),
                            "time_unix_nano": point.get("timeUnixNano"),
                        }
                    )
