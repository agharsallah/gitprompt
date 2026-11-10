"""Sources: the AI tools GitPrompt captures from.

spec.py        the Spec model, and loading bundled/ + user override YAML files
probe.py       read-only checks on this machine: installed? which version? does policy allow us?
connection.py  connect / disconnect workflow (policy gate, dry-run diff, record of added values)
installer.py   the JSON config-file mechanics connection.py is built on
bundled/       the YAML specs shipped with GitPrompt
"""
