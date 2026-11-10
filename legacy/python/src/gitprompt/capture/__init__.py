"""Capture: turning hook calls and telemetry exports into stored prompts and events.

pipeline.py  decode (by spec.format) -> map (spec.map) -> observe (git, tool version) -> store
otlp.py      decoder for OTLP/HTTP JSON export requests
"""
