"""The Python entry point hands every command to the Rust binary (docs/adr/0023-rust-backend.md)."""

import os
import stat
import sys

import pytest

from gitprompt import cli


def _binary(path, native=True):
    path.write_bytes(b"\xcf\xfa\xed\xfe binary" if native else b"#!/usr/bin/env python3\n")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def test_an_explicit_binary_wins(tmp_path):
    binary = _binary(tmp_path / "gitprompt")
    assert cli.rust_binary({"GITPROMPT_BIN": str(binary), "PATH": ""}) == binary


def test_a_missing_explicit_binary_is_not_replaced_by_a_guess(tmp_path):
    assert cli.rust_binary({"GITPROMPT_BIN": str(tmp_path / "nope"), "PATH": ""}) is None


def test_a_python_script_on_the_path_is_not_the_rust_binary(tmp_path, monkeypatch):
    monkeypatch.setattr(cli, "REPO", tmp_path)  # no repository builds
    monkeypatch.setattr(cli.Path, "home", lambda: tmp_path)  # no ~/.cargo/bin
    _binary(tmp_path / "gitprompt", native=False)
    assert cli.rust_binary({"PATH": str(tmp_path)}) is None
    native = tmp_path / "bin"
    native.mkdir()
    _binary(native / "gitprompt")
    assert cli.rust_binary({"PATH": os.pathsep.join([str(tmp_path), str(native)])}) == native / "gitprompt"


def test_commands_are_handed_over_unchanged(tmp_path, monkeypatch, capsys):
    binary = _binary(tmp_path / "gitprompt")
    calls = []
    monkeypatch.setattr(cli.os, "execv", lambda path, args: calls.append((path, args)))
    cli.delegate(["compact", "--apply"], {"GITPROMPT_BIN": str(binary)})
    assert calls == [(binary, [str(binary), "compact", "--apply"])]
    assert "deprecated" in capsys.readouterr().err


def test_without_a_binary_it_says_how_to_build_one(monkeypatch, capsys, tmp_path):
    monkeypatch.setattr(cli, "rust_binary", lambda environ=None: None)
    with pytest.raises(SystemExit) as exit:
        cli.delegate(["serve"])
    assert exit.value.code == 2
    assert "cargo build" in capsys.readouterr().err


def test_legacy_runs_the_python_cli_without_the_flag(monkeypatch):
    seen = []
    monkeypatch.setattr(cli, "legacy_main", lambda: seen.append(list(sys.argv)))
    monkeypatch.setattr(sys, "argv", ["gitprompt", "--legacy", "check"])
    cli.main()
    assert seen == [["gitprompt", "check"]]
