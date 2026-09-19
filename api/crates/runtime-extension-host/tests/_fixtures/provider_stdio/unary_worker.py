#!/usr/bin/env python3
"""Stable stdio fixture: no executable files are written during tests."""
import json
import os
import sys
import time

cursors = {"session-b": "cursor-b"}
for index, line in enumerate(sys.stdin):
    request = json.loads(line)
    payload = request["input"]
    mode = payload["mode"]
    if mode == "eof":
        sys.exit(0)
    if mode == "late":
        time.sleep(1)
        print('{"ok":true,"result":"late"}', flush=True)
    elif mode == "raw":
        print(payload["response"], flush=True)
    elif mode == "rejections":
        if index < 3:
            message = ["command expired", "generation mismatch", "connection absent"][index]
            result = {"ok": False, "error": {"kind": "provider_transport_unavailable", "message": message}}
        else:
            result = {"ok": True, "result": {"cursor": cursors["session-b"], "pid": os.getpid()}}
        print(json.dumps(result), flush=True)
    else:
        raise ValueError("unknown fixture mode")
