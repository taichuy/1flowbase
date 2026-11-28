# Quality gate tiers

| Tier | Entry point | Evidence |
| --- | --- | --- |
| Level 1 — feedback | `node scripts/node/verify-repo.js level1` | Diff advisory, repo and i18n hygiene, Vite lazy boundaries, Rust static scan and format check, and focused routing/resource script tests. No Rust test build or link. Run `test-frontend.js fast <test-file>` for a focused UI regression. |
| Level 2 — PR | `node scripts/node/verify-repo.js level2 --base-ref <full-sha>` or `verify.yml` | Compare the base and candidate trees without rename detection, so source and destination owners both count. Run affected frontend and backend scopes, reverse-dependent check shards, and affected foundation fast packs. Global changes or an unavailable baseline select every scope. CI always requires a candidate-bound foundation receipt, including for a no-impact diff. |
| Level 3 — release | `node scripts/node/verify-repo.js level3` (`verify-ci.js`) or `quality-gate.yml` with `scope=ci` | Full regression and coverage locally; CI additionally requires all release component receipts. API coverage builds an instrumented nextest archive once. Four shards run those binaries, and merge validates the frozen SHA, archive identity, complete nonoverlapping test inventory, and profile counts before reporting. |

Existing `verify-repo.js tooling`, `frontend`, `frontend-pr`, `backend`, and `all` targets remain available. The PR frontend slice includes the shared frontend contract tests; do not run `test-contracts.js` again in the same slice.

Locally, the worker budget is `max(1, floor(0.60 × effective CPU count))`. Workspace testing divides it across Turbo tasks and their Vitest workers. `.1flowbase.verify.local.json` can lower worker counts; CI ignores this file and uses runner capacity. The Linux resource profile additionally caps its aggregate memory at the smaller of 8 GiB and effective capacity. Direct shell commands outside that profile are not memory-isolated by the CLI configuration.

Frontend stable, fast, coverage and page-regression commands share one page-test inventory in `scripts/node/run-frontend-vitest/layers.js`. Exclusions use globs; the page lane uses Vitest positional file or directory filters. Explicit Level 1 file requests bypass those exclusions. Level 2 runs the Frontstage page-selection and Agent Flow variable-cascader contracts in the existing PR smoke process.

The shared i18n hygiene scanner used by local feedback and the PR/release tooling gates tracks JSON line locations during its token scan. It formats source snippets only for reported findings, preserving duplicate-key and naming diagnostics and the existing warning/error policy.
