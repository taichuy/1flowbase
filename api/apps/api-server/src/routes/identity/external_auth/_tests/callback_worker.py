#!/usr/bin/env python3
"""Controlled verifier transport; no live GitHub/network dependency.

The response barrier sits after the provider result has been computed, before it
is delivered to Host. It models the last egress response having completed while
an admitted worker still holds an old result, which a final Host grant check must reject.
"""
import json
import pathlib
import sys
import time
import urllib.parse

for line in sys.stdin:
    envelope = json.loads(line)
    if envelope.get("kind") != "call":
        continue
    call_id = envelope["call_id"]
    assert call_id.isdecimal() and str(int(call_id)) == call_id and int(call_id) > 0
    payload = envelope["request"]["input"]["input_payload"]
    operation = payload["operation"]
    if operation == "begin":
        query = urllib.parse.urlencode({
            "state": payload["state"], "redirect_uri": payload["callback_url"],
            "code_challenge": payload["code_challenge"], "code_challenge_method": "S256",
            "response_type": "code",
        })
        result = {"type": "authorization", "authorization_url": "https://provider.example/authorize?" + query}
    elif operation == "complete":
        result = {"type": "verified_identity", "subject_type": "provider-user-id", "subject_value": "42",
                  "issuer": "https://provider.example", "realm": None,
                  "profile": {"email": "external-fixture@example.com", "email_verified": True}}
        if payload["code"].startswith("pause_after_response:"):
            # Keep synchronization files outside verified installation bytes.
            path = pathlib.Path(payload["code"].split(":", 1)[1])
            path.with_suffix(".started").write_text("response-computed")
            deadline = time.monotonic() + 20
            while not path.with_suffix(".release").exists():
                if time.monotonic() >= deadline:
                    raise RuntimeError("fixture response barrier timed out")
                time.sleep(0.005)
    else:
        # A bound identity with self-registration disabled must never reach this.
        raise RuntimeError("unexpected registration eligibility invocation")
    print(json.dumps({"protocol": "stdio_json_multiplex_v1", "kind": "response", "call_id": call_id,
                      "response": {"ok": True, "result": result}}), flush=True)
