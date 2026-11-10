"""GitPrompt: keep every prompt you write to an AI tool, locally.

Packages, one per domain (dependencies point down this list, never up):

    api/       HTTP layer: FastAPI app, dependencies, live-update bus, thin routers
    insights/  values derived on read: prompt stats, turn/session summaries, secrets, field catalog
    capture/   request body -> decoded payloads -> mapped prompt or event -> store
    storage/   SQLite persistence of prompts, events and annotations
    sources/   what GitPrompt knows about each tool: YAML specs, probing, connecting
    core/      shared kernel: settings, domain errors, JSON payload helpers, git facts
"""
