from __future__ import annotations
import json
from pathlib import Path

from src.events.event_base import Event

CONTENT_DIR = Path(__file__).resolve().parents[2] / "content" / "events"


def load_events(content_dir: Path = CONTENT_DIR) -> list[Event]:
    events: list[Event] = []
    for path in sorted(content_dir.glob("*.json")):
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        events.extend(Event.from_dict(entry) for entry in data)
    return events
