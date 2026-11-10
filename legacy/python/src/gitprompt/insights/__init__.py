"""Insights: everything GitPrompt derives from stored data on read (never stored, so better rules
apply to old prompts too).

    derivations.py  registries of derived values: prompt stats, turn summary, session facts
    origin.py       who produced a prompt (spec rules) and the text the user typed
    redaction.py    finding and hiding credentials
    catalog.py      field catalog: fields seen in payloads next to the fields each tool documents
"""
