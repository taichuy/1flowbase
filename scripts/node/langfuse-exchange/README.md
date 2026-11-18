# Langfuse managed-plugin acceptance

This finite scenario targets an independently running exact-source host and installed
public `@taichuy/langfuse` plugin. Supply matching host, static web and plugin artifacts before starting the scenario;
the one-off evidence workflows are retired. Do not run local Cargo or cold frontend builds.

Use `install.cjs`'s `install` function with an explicit package, parsed public manifest,
`apiBaseUrl`, `credentialsRoot`, and `outDir`. It exercises the existing package
intake, explicit contribution grants and system activation APIs. It permits only
known unsigned development-package risks and saves no credentials.

Then run:

```sh
node scripts/node/langfuse-exchange/scenario.cjs \
  --api-base-url http://127.0.0.1:7801 \
  --web-base-url http://127.0.0.1:3102 \
  --credentials-root /absolute/path/to/1flowbase \
  --langfuse-private-file /absolute/path/to/private-project-credentials.json \
  --langfuse-storage-private-file /absolute/path/to/private-langfuse-storage.json \
  --out-dir /absolute/path/to/candidate/tmp/test-governance/langfuse-plugin/browser
```

The private project file provides `base_url`, `public_key`, `secret_key`, and
`project_id`; browser storage is an existing authenticated Langfuse session. Never
commit either file. Authentication uses the host's temporary session owner and is
revoked in `finally`.

The scenario creates two labeled Agent Logs applications, ingests a controlled
21-event / 3-task / 2-conversation source, and reads retained Agent Flow history
without model calls. `--native-application-id` optionally chooses that history.
On desktop 1440×1000 and mobile 390×844 it creates two owned table instances with
independent credential pairs, reloads both, edits the first with blank credentials,
checks its real connection, verifies the invalid second pair fails independently,
deletes the second, and checks the first still works. It switches English and
Simplified Chinese through the real global LanguageChromeMenu and observes its
`PATCH /api/console/me/meta`; language URL hints and plugin language buttons do not
drive these assertions. Screenshots include the shared desktop table/mobile list
and the Chinese instance drawer. Each row's export/import drawer explicitly submits
that row's `instance_id`. The scenario then
exports both sources by date, waits for remote persistence, imports into the chosen
Agent Logs application, repeats import, compares exact original source facts, and
captures actual pages. It also imports the current synthetic traces by date and checks source/target/date label associations and unique form IDs. Source and target selection use the real label-associated combobox, supported ArrowDown/Enter input, and the active option UUID. No private CSS or virtual scroll state is assumed, and no hidden UI state is set. The source application keys are revoked independently on exit (including normal DELETE204 responses); labeled
applications and imported history remain available for manual inspection.

The same batch checks real OpenAPI discovery against every operation in the live
full document (no fixed operation count), selects the GET projects and observations
operations by method/path metadata, checks operation schemas and local reference
closure, and executes limited reads plus an invalid limit. Actual upstream status,
JSON/error body, authoritative base64 bytes and absence of credentials are checked.
No availability probe, alternate endpoint or retry is used by these new operations.
It creates an ephemeral user PAT with the current actor role, chooses an existing
enabled non-browser MCP instance, and calls actual `/mcp/{instance_id}` JSON-RPC
`mcp_list`, `mcp_get`, and `mcp_call` for all four fixed tools. Oversized responses
are read through documented `mcp_result` continuation/projection without rerunning
the business operation.

Finally it independently revokes the PAT and verifies `revoked=true` from the user
key list, deletes only owned plugin instances, checks preexisting instance DTOs
remain unchanged, restores only the original owned locale preference using current
metadata, disposes both temporary session owners, and removes browser storage.
`receipt.json` checkpoints each created instance, application key ID and user key ID
immediately, plus the original locale preference, so the Root's independent cleanup
can recover after interruption. Tokens and project keys are never written to receipts.
The fixture does not mutate legacy/default connections or create native execution.

Receipts and screenshots go beneath `tmp/test-governance/`. A resource supervisor
must stop only this task's runtime/browser processes if system CPU, physical memory,
or disk occupancy reaches 90%. Ports 7600 and 7800 are protected and rejected.

Existing standard OTLP codec, authorization, native archive and source-reader
regressions remain in the repository (the one-off OTLP workflow is retired); the plugin's own SDK callback and mapping
fixtures run in the official repository's `langfuse-ci.yml`.

Remote persistence is verified by complete, trace-bound observations v2 cursor reads,
with a narrow older-deployment capability fallback to the numbered observations API.
Duplicate remote bookkeeping versions are compared by semantic facts; foreign traces,
real conflicts and pagination loops fail acceptance. HTTP200 or an enqueue receipt alone
does not pass the persistence check. External screenshots open the exact current synthetic
Trace detail and timestamp, not a project-wide history table. Keys are masked, and final
publication still requires visual inspection. All original21-fact/raw/dedup assertions remain.

The helper regression tests are `node --test scripts/node/langfuse-exchange/_tests/*.test.cjs`.
