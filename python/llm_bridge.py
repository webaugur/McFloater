#!/usr/bin/env python3
"""Ollama sidecar bridge for Floaty McFloater dialog (Phase 3)."""

from __future__ import annotations

import json
import sys
from typing import Any


def main() -> None:
    """Stub: read JSON lines from stdin, echo placeholder replies."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req: dict[str, Any] = json.loads(line)
        except json.JSONDecodeError:
            continue
        user = req.get("user", "")
        reply = f"G-g-great question about {user!r}. Catch the wave!"
        print(json.dumps({"reply": reply}), flush=True)


if __name__ == "__main__":
    main()
