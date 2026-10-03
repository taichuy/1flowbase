# Harness-only same-binary A/A packet (draft)
Owner: cloud gateway owner; root coordinates the only dispatch. Issue2216. No product paths owned.

AC: same60308 source/API tree/baseline binary in both fixtures; exact reused archive/package hashes; six client marks distinguish expected256th delta/protocol terminal/HTTP body EOF; missing/out-of-order marks remain invalid, not repaired; one phase-outside server-side white-list attempt query restricted to measured UUIDs/app/database; no full body/metrics retained; own observer CPU/bytes/query cost recorded. Server innerEOF/archive completion/dispatcher spans explicitly unobserved.

Budget: AA1/AA2, C8,512KiB/128history,256deltas10ms; each8warm+8measured+16direct+16native=48 primaryHTTP, total96 (excluding owned setup management); extra2 read-onlySQL total; no implicit retry/real model. Reuse107MB build artifact and verified177MB baseline binary on Actions, no compiler setup/build anywhere.

Pure tests implemented in assembly (40 total Node cases plus7 unchanged sampler cases): timeline exact six-clock normal terminal/body-end separation; earlyEOF, missing/extra delta, ordering and duplicate terminal rejection; UUID/app whitelist and SQL injection refusal; actual SQL only returns bounded summary/receipt fields, no input/answer/headers/user_account; controlled private-field data proves omission; bound rows/bytes and absent summary disclosure; reuse source/tree/binary/package mismatch rejection.

Source evidence: production compatibility_interface.rs:1017-1067 emits terminal then awaits Kernel completion holding body sender; client_observer.rs:100-149 awaits recorder.record before each frame and recorder.complete before EOF; recorder scope/complete semantics unchanged. Provider close dispatcher is an existing possible finalization contributor; same module in A/B. Legacy SSE close logger/event-forwarding448-479 are cfg(test) and excluded.

Build/read/control stop: any hash/source mismatch, scope escape, unexpected extra model call, denied access, missing terminal/256deltas/native succeeded, observer export over512KiB/SQL timeout, or change required to product semantics/permissions stops. Diagnostic failure is not retried unchanged. No optimization acceptance/PR from this probe.

Status: timeline/collector/reuse proof/runner/workflow implemented locally. Pure40 Node+7 sampler and YAML/syntax/whitespace checks passed; independent source review passed. Query SQL has not run on real PostgreSQL yet; server innerEOF/archive-done/dispatcher remain unobserved. No push/dispatch. Remote execution remains pending; publication/dispatch is coordinated by root against the final commit SHA.

Observer cost is not claimed zero: only six monotonic clock calls per fixed256-delta stream, plus a small JS counter/two conditions per parsed delta; all diagnostic JSON bytes/phase driver CPU and one query elapsed/export bytes are retained. Two A/A clusters are a localization probe, not a confidence interval or performance-acceptance experiment.

Independent review fixed the tied-clock ordering gap with six mark ordinals and two controlled negative cases; six clock reads retained. Cached authorized baseline and package bytes were hashed through the actual reuse-proof helper, without execution. First actual PostgreSQL timing SELECT will be on Actions; missing/invalid summaries stop rather than imply successful timing observation.
