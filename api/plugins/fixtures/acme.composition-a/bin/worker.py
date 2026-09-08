#!/usr/bin/env python3
"""Finite stdio fixture; marker files provide an observable in-flight barrier."""
import json
import pathlib
import sys
import time

request = json.load(sys.stdin)
body = request["input"]
marker = body.get("input_payload", {}).get("fixture_barrier")
if marker:
    root = pathlib.Path(marker)
    root.with_suffix(".started").write_text("admitted")
    end = time.monotonic() + 20
    while not root.with_suffix(".release").exists():
        if time.monotonic() >= end:
            raise TimeoutError("fixture barrier expired")
        time.sleep(0.01)
print(json.dumps({"ok": True, "result": {"worker": body["contribution_code"], "input": body["input_payload"]}}))
