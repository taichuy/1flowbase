# GitHub Automation

This directory owns GitHub Actions automation for repository quality gates.

## Files

| Path | Purpose |
| --- | --- |
| `.github/workflows/verify.yml` | Automatic merge CI for `pull_request` and `push` to `main` / `latest`; runs lightweight repo tooling, frontend PR, and backend static/fmt/check gates, updates one PR report comment for same-repository pull requests, then publishes one aggregate issue only for `latest` pushes. |
| `.github/workflows/quality-gate.yml` | Manual and nightly quality gate run; full `ci` scope runs component gates, coverage gates, and container image security in parallel before one aggregate Issue report. |
| `.github/workflows/foundation-contracts.yml` | Reusable four-foundation fast contract executor called by `verify.yml` and full `quality-gate.yml`; emits candidate-bound component evidence and remains manually dispatchable for focused reruns. |
| `.github/workflows/ai-gateway-concurrency.yml` | Reusable and manually runnable full AI Gateway protocol conformance gate; joins full manual/nightly `ci` aggregation and does not run for every pull request. |
| `.github/workflows/container-images.yml` | Container image CD for `web`, `api-server`, and `plugin-runner`; builds scan-candidate GHCR tags, runs Trivy admission scans, promotes passing images to version and `latest` tags, then uploads artifact-only CD quality gate evidence. |
| `.github/actions/quality-gate/action.yml` | Reusable repository-local action used by CI, manual, and nightly quality gates. |

## Automatic CI

`verify.yml` runs automatically on:

- `pull_request`
- `push` to `beta`
- `push` to `main`
- `push` to `latest`

It runs lightweight local Quality Gate Action jobs in parallel:

```yaml
scope: repo-tooling
scope: repo-frontend-pr
scope: repo-backend-static
scope: repo-backend-fmt
scope: repo-backend-check-{core-libs,runtime-storage,apps}
component: foundation-contracts
```

The `foundation-contracts` component is produced by the reusable four-foundation workflow
and is downloaded by the same final `verify` aggregate as the repo component reports.

The `repo-frontend-pr` scope runs the Vite lazy dependency static gate, web lint, a
compact frontend PR smoke suite, and the app build. Full app Vitest, page regression,
style-boundary, coverage, and backend consistency evidence stay in nightly or manual
full quality gates.

The `repo-tooling` scope starts with `gate-router`, a non-blocking advisory that
prints related quality gate suggestions for the current branch diff. It also
includes `repo-hygiene`, which writes `tmp/test-governance/repo-hygiene.json`
with debt-marker, weak-assertion, duplicate-test-title, file-size, and
directory-pressure findings. It also runs `security-risk`, which writes
`tmp/test-governance/security-risk.json` for changed dependency, lockfile,
communication, CI, Docker, deploy, proxy, plugin, and runtime execution-path
risks. Advisory findings remain warnings; focused tests still fail the repo gate.

React Doctor remains outside the automatic PR merge blockers in `verify.yml`.
It runs as a parallel component in scheduled quality gates and manual `scope: ci`
runs, so the aggregate report includes structural frontend debt without requiring
a second dispatch. Run `scope: repo-frontend-react-doctor` directly when you need
focused structural frontend debt evidence:

```yaml
scope: repo-frontend-react-doctor
run: npm exec --yes --package react-doctor@0.2.16 -- react-doctor web/app --diff <parent-sha> --no-score --fail-on warning --verbose --no-color
```

Current React Doctor structural debt is kept in `web/app/doctor.config.json` as
narrow per-file rule overrides for those stricter runs. The gate writes
`tmp/test-governance/react-doctor.*` alongside the standard quality gate report
artifact. By default the gate resolves the current commit's parent SHA and
blocks warnings introduced by that commit. Set
`REACT_DOCTOR_DIFF_BASE` to an older ref, such as `origin/main`, for a broader
structural debt audit.

The final aggregate job downloads the component artifacts and publishes a single
report with:

```yaml
INPUT_PUBLISH_ISSUE: ${{ github.event_name == 'push' && github.ref == 'refs/heads/latest' }}
INPUT_PUBLISH_PR_COMMENT: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository }}
```

Automatic CI updates one fixed-marker PR comment for same-repository pull requests and
creates a GitHub Issue only for `latest` branch pushes. It also uploads the merged
`tmp/test-governance` directory as the `test-governance-artifacts` artifact. The report body
includes the aggregate result summary, component status table, advisory warning status,
security-risk summary, evidence paths, and a failure excerpt when a
component gate fails.
Use the artifact for full logs and security-risk finding details.
Runs use branch-level concurrency, so a newer push cancels an older in-progress quality gate
for the same branch before stale runs can publish or close quality issues.

## Foundation Contract Evidence

`foundation-contracts.yml` adds a second routing axis inside the existing quality gates without changing merge authority:

```text
changed files -> affected foundation fast packs -> candidate-bound receipt
              -> complete provider/browser/migration matrices deferred to nightly/manual
```

`verify.yml` calls the changed-file fast pack for PR and `beta/main/latest` evidence. Scheduled
and manual full `quality-gate.yml` calls all four packs and includes the resulting
`foundation-contracts` component in its unified aggregate report. The reusable workflow has no
standalone PR or push trigger; `workflow_dispatch` exists only for focused diagnosis.

The receipt is written below `tmp/test-governance/foundation-contracts/` and records the
candidate SHA, trigger reasons, executed packs, warnings, uncovered items, and deferred
evidence. Warnings remain visible and advisory; only an explicit failed component, non-zero
exit code, or missing selected component receipt fails the aggregate.

Every fast component has a 40-minute timeout, while route and aggregate jobs use 5 minutes,
so the workflow execution path remains below one hour. Full AI Gateway conformance has a
55-minute job timeout and remains available through nightly/manual orchestration.

When fewer than three foundations repeatedly fail, first run the corresponding local pack,
then dispatch `foundation-contracts.yml` with that single `foundation`, and only after it is
green rerun `auto` or `all`. These checks provide evidence for administrators; they are not
configured as required checks and do not alter branch protection or repository rulesets.

## Container Image CD

`container-images.yml` publishes the `web`, `api-server`, and `plugin-runner` images for
`latest` pushes and for the selected `workflow_dispatch` component. Enabled components first
build native artifacts outside the publish job: `web` uploads per-architecture `dist`
artifacts, and the Rust components upload per-architecture binaries. Publish jobs then build
multi-platform scan-candidate tags named `scan-<run_id>-<run_attempt>-<sha>` from those
prebuilt artifacts. The workflow does not push the official version tag or `latest` tag from
the build step.

Before promotion, Trivy scans the candidate image with a pinned `aquasecurity/trivy-action`
commit for action version `v0.36.0`; the action installs Trivy `v0.70.0`. `HIGH` and
`CRITICAL` findings are written as warning evidence with `exit-code: "0"`. Reports are
uploaded from:

```text
tmp/test-governance/trivy-${component}-high.json
tmp/test-governance/trivy-${component}-critical.json
```

After the publish jobs finish, the report job downloads the Trivy artifacts and runs
`scope: container-images` through the local Quality Gate Action with `publish_issue: "false"`.
Container CD reports are artifact-only so vulnerability and system-error details stay in
Actions artifacts instead of GitHub Issues. The local reporter writes:

```text
tmp/test-governance/container-image-security.md
tmp/test-governance/container-image-security.json
```

The workflow promotes the scanned candidate manifest to `${image_tag}` and `latest` with
`docker buildx imagetools create` after the warning reports are captured.

## Manual And Nightly Quality Gate

`quality-gate.yml` is triggered from GitHub Actions with `workflow_dispatch` and by a daily
schedule at 18:00 UTC, which is 02:00 Asia/Shanghai.

Recommended first run:

```text
scope: ci
report_type: ci
environment: leave empty
```

For manual `scope: ci`, runs use the full quality gate shape: repo tooling,
full repo frontend, React Doctor, backend static/fmt/package shards, backend app test
package shards, backend consistency, frontend coverage, backend coverage package shards,
state protocols, container image security, the four-foundation contract receipt, and
mock-backed AI Gateway protocol conformance
run as separate jobs. Scheduled `scope: ci`
runs use the same component set.
An aggregate job downloads their artifacts, publishes one Issue report, and uploads
`test-governance-artifacts`.
This keeps wall time close to the slowest component gate instead of the sum of all gates.
Each component job publishes `publish_issue: "false"`; only the aggregate job publishes the
final report with `publish_issue: "true"`.

Full AI Gateway conformance remains independently dispatchable and does not run for every
pull request. Pull requests use the affected fast pack from `foundation-contracts.yml`.
The full nightly/manual gate invokes the same reusable workflow,
downloads its standard component report, and lists
`ai-gateway-protocol-conformance` explicitly in the aggregate component table. It never
uses real Provider credentials or local client binaries.

For narrower dispatch scopes such as `repo-frontend-pr`, `repo-frontend`, `repo-frontend-react-doctor`, `repo-backend`, `backend-consistency`,
`coverage-backend`, or `container-images`,
`quality-gate.yml` runs one targeted job and publishes that single-scope report directly.
Manual runs share the same target-branch concurrency group as automatic quality gates.
Scheduled runs target `latest`, use `scope: ci`, and set `environment: nightly-latest`.

## Scope Options

| Scope | Command |
| --- | --- |
| `ci` | GitHub workflow only: parallel repo slices + `backend-consistency` + coverage slices, then `github-quality-gate-aggregate.js` |
| `repo` | `node scripts/node/verify-repo.js` |
| `repo-tooling` | `node scripts/node/verify-repo.js tooling` |
| `repo-frontend-pr` | `node scripts/node/verify-repo.js frontend-pr` |
| `repo-frontend` | `node scripts/node/verify-repo.js frontend` |
| `repo-frontend-react-doctor` | `node scripts/node/react-doctor-gate.js` |
| `repo-backend` | `node scripts/node/verify-repo.js backend` |
| `repo-backend-static` | `node scripts/node/verify-backend.js static` |
| `repo-backend-fmt` | `node scripts/node/verify-backend.js fmt` |
| `repo-backend-image-llm-vision` | `node scripts/node/verify-backend.js image-llm-vision` |
| `repo-backend-clippy-core-libs` | `node scripts/node/verify-backend.js clippy core-libs` |
| `repo-backend-clippy-runtime-storage` | `node scripts/node/verify-backend.js clippy runtime-storage` |
| `repo-backend-clippy-apps` | `node scripts/node/verify-backend.js clippy apps` |
| `repo-backend-test-core-libs` | `node scripts/node/verify-backend.js test core-libs` |
| `repo-backend-test-runtime-storage` | `node scripts/node/verify-backend.js test runtime-storage` |
| `repo-backend-test-control-plane` | `node scripts/node/verify-backend.js test control-plane` |
| `repo-backend-test-api-server` | `node scripts/node/verify-backend.js test api-server` |
| `repo-backend-test-plugin-runner` | `node scripts/node/verify-backend.js test plugin-runner` |
| `repo-backend-check-core-libs` | `node scripts/node/verify-backend.js check core-libs` |
| `repo-backend-check-runtime-storage` | `node scripts/node/verify-backend.js check runtime-storage` |
| `repo-backend-check-apps` | `node scripts/node/verify-backend.js check apps` |
| `backend` | `node scripts/node/verify-backend.js` |
| `backend-consistency` | `node scripts/node/cli/verify-backend-consistency.js` |
| `coverage` | `node scripts/node/verify-coverage.js all` |
| `coverage-frontend` | `node scripts/node/verify-coverage.js frontend` |
| `coverage-backend` | `node scripts/node/verify-coverage.js backend` |
| `coverage-backend-control-plane` | `node scripts/node/verify-coverage.js backend control-plane` |
| `coverage-backend-storage-postgres` | `node scripts/node/verify-coverage.js backend storage-postgres` |
| `coverage-backend-api-server` | `node scripts/node/verify-coverage.js backend api-server` |
| `container-images` | `node scripts/node/cli/container-image-security.js` |

Use `ci` for the full online repository quality gate. The local `node scripts/node/verify-ci.js`
entry still runs the same gates serially for environments that need one local command, but the
GitHub workflow intentionally splits `ci` across jobs for faster feedback and clearer artifacts.
Use narrower scopes only when debugging or when a faster targeted report is enough. Do not run
the backend consistency scope locally unless explicitly requested, because it exercises
database-backed Rust suites.

## Report Type

| `report_type` | Use When |
| --- | --- |
| `ci` | Code quality, regression, or repository validation. |
| `cd` | Deployment or release validation. |

For `cd` reports, set `environment` to a value such as `staging` or `production`.

## Evidence

The Quality Gate Action writes:

- `tmp/test-governance/quality-gate.latest.log`
- `tmp/test-governance/quality-gate-report.md`
- `tmp/test-governance/quality-gate-report.json`
- `tmp/test-governance/repo-hygiene.json` for `repo`, `repo-tooling`, and `ci`
- `tmp/test-governance/backend-consistency-targets.json` for `ci` and `backend-consistency`

Existing warning, coverage, screenshot, and QA evidence files remain under
`tmp/test-governance/`.
For `ci` and `backend-consistency` scopes, the report also includes the current backend
consistency target results: label, Cargo package, Rust test filter, status, duration,
passed count, and failed count. If the target result artifact is unavailable, the report
falls back to the static target registry with `not_run` status.

## Setup Order

Workflows must install pnpm before enabling `setup-node` pnpm cache:

```yaml
- uses: pnpm/action-setup@v5
- uses: actions/setup-node@v5
  with:
    cache: pnpm
```

Putting `setup-node` before `pnpm/action-setup` can fail with `Unable to locate executable
file: pnpm`.

Workflows opt JavaScript actions into GitHub's Node 24 runtime with:

```yaml
FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
```

This keeps hosted-action runtime annotations aligned with the repository's Node 24 test runtime.
