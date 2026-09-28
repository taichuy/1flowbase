#!/usr/bin/env python3
import json
import os
import sys
import time

for line in sys.stdin:
    request = json.loads(line)
    data = request["input"]
    with open(data["marker"], "a") as marker:
        marker.write(data["mode"] + "\n")
    if data["mode"] == "wait":
        while not os.path.exists(data["release"]):
            time.sleep(0.005)
    if request["method"] == "invoke":
        response = {"type": "result", "result": {"final_content": "done", "finish_reason": "stop"}}
    else:
        response = {"ok": True, "result": {"mode": data["mode"]}}
    print(json.dumps(response), flush=True)
