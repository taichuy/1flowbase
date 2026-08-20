---
decision_policy: verify_before_decision
date: 2026-08-19 23
status: approved
---

# Plugin upload platform compatibility implementation approved

The 1flowbase team will implement the approved balanced Single Issue: the backend validates the uploaded package and its runtime platform before installation, while the model-provider upload UI constrains long filenames and presents actionable Chinese failures.

This is necessary because an `linux-arm64` package can be accepted on the current `x86_64` Linux host even though its runtime executable cannot run. The backend remains the sole owner of package validation and platform compatibility; the frontend must not infer an installation result from the filename. A browser transport failure must be rendered as an operational upload error instead of the raw `Failed to fetch` string.

The user supplied both `linux-amd64` and `linux-arm64` ChatGPT packages for verification. The arm64 package is an explicit rejection fixture, and the amd64 package is the successful-path fixture. No external deadline was set; implementation starts immediately after this approval.
