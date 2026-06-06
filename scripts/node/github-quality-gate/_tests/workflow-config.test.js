const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function readTextFile(...segments) {
  return fs.readFileSync(path.join(repoRoot, ...segments), "utf8").replace(/\r\n/gu, "\n");
}

function readVerifyWorkflow() {
  return readTextFile(".github", "workflows", "verify.yml");
}

function readQualityGateWorkflow() {
  return readTextFile(".github", "workflows", "quality-gate.yml");
}

function readContainerImagesWorkflow() {
  return readTextFile(".github", "workflows", "container-images.yml");
}

function readApiServerDockerfile() {
  return readTextFile("docker", "api-server.Dockerfile");
}

function readQualityGateAction() {
  return readTextFile(".github", "actions", "quality-gate", "action.yml");
}

function readGitHubAutomationDocs() {
  return readTextFile(".github", "GITHUB_AUTOMATION.md");
}

function readReactDoctorConfig() {
  return JSON.parse(
    fs.readFileSync(
      path.join(repoRoot, "web", "app", "doctor.config.json"),
      "utf8",
    ),
  );
}

function readWebAppViteConfig() {
  return readTextFile("web", "app", "vite.config.ts");
}

function extractPushBranches(workflow) {
  const match = workflow.match(
    /push:\n\s+branches:\n(?<branches>(?:\s+- .+\n)+)/u,
  );
  assert.ok(match, "verify workflow must declare push branches");

  return match.groups.branches
    .split(/\r?\n/u)
    .map((line) => line.trim().replace(/^- /u, ""))
    .filter(Boolean);
}

test("verify workflow runs on main and latest but only publishes quality reports on latest pushes", () => {
  const workflow = readVerifyWorkflow();

  assert.deepEqual(extractPushBranches(workflow), ["main", "latest"]);
  assert.match(
    workflow,
    /concurrency:\n\s+group: verify-\$\{\{ github\.ref_name \}\}\n\s+cancel-in-progress: true/u,
  );
  assert.match(
    workflow,
    /INPUT_PUBLISH_ISSUE: \$\{\{ github\.event_name == 'push' && github\.ref == 'refs\/heads\/latest' \}\}/u,
  );
  assert.match(
    workflow,
    /INPUT_PUBLISH_PR_COMMENT: \$\{\{ github\.event_name == 'pull_request' && github\.event\.pull_request\.head\.repo\.full_name == github\.repository \}\}/u,
  );
  assert.match(workflow, /INPUT_PR_NUMBER: \$\{\{ github\.event\.pull_request\.number \}\}/u);
  assert.doesNotMatch(workflow, /INPUT_PUBLISH_ISSUE: .+refs\/heads\/main/u);
});

test("verify workflow runs lightweight merge gates before one aggregate report", () => {
  const workflow = readVerifyWorkflow();

  assert.match(workflow, /repo-tooling-gate:\n\s+runs-on: ubuntu-latest/u);
  assert.match(
    workflow,
    /repo-tooling-gate:[\s\S]*?fetch-depth: 0/u,
  );
  assert.match(workflow, /repo-frontend-gate:\n\s+runs-on: ubuntu-latest/u);
  assert.match(workflow, /repo-backend-gate:\n\s+runs-on: ubuntu-latest/u);
  assert.match(workflow, /fail-fast: false/u);
  assert.match(workflow, /- repo-backend-static/u);
  assert.match(workflow, /- repo-backend-fmt/u);
  assert.match(workflow, /- repo-backend-check-core-libs/u);
  assert.match(workflow, /- repo-backend-check-runtime-storage/u);
  assert.match(workflow, /- repo-backend-check-apps/u);
  assert.doesNotMatch(workflow, /- repo-backend-clippy-core-libs/u);
  assert.doesNotMatch(workflow, /- repo-backend-test-control-plane/u);
  assert.doesNotMatch(workflow, /- repo-backend-test-api-server/u);
  assert.doesNotMatch(workflow, /- repo-backend-test-plugin-runner/u);
  assert.doesNotMatch(workflow, /backend-consistency-gate:/u);
  assert.doesNotMatch(workflow, /coverage-frontend-gate:/u);
  assert.doesNotMatch(workflow, /coverage-backend-gate:/u);
  assert.match(
    workflow,
    /verify:\n\s+needs:\n\s+- repo-tooling-gate\n\s+- repo-frontend-gate\n\s+- repo-backend-gate/u,
  );
  assert.match(workflow, /scope: repo-tooling/u);
  assert.match(workflow, /scope: repo-frontend-pr/u);
  assert.match(workflow, /scope: \$\{\{ matrix\.scope \}\}/u);
  assert.match(
    workflow,
    /start_postgres: "false"/u,
  );
  assert.match(workflow, /name: test-governance-repo-tooling/u);
  assert.match(workflow, /name: test-governance-repo-frontend-pr/u);
  assert.match(workflow, /name: test-governance-\$\{\{ matrix\.scope \}\}/u);
  assert.match(workflow, /INPUT_EXPECTED_SCOPES: repo-tooling,repo-frontend-pr,repo-backend-static,repo-backend-fmt,repo-backend-check-core-libs,repo-backend-check-runtime-storage,repo-backend-check-apps/u);
  assert.match(
    workflow,
    /INPUT_PUBLISH_PR_COMMENT: \$\{\{ github\.event_name == 'pull_request' && github\.event\.pull_request\.head\.repo\.full_name == github\.repository \}\}/u,
  );
  assert.match(workflow, /merge-multiple: false/u);
  assert.match(
    workflow,
    /node scripts\/node\/cli\/github-quality-gate-aggregate\.js/u,
  );
});

test("verify workflow keeps React Doctor out of automatic merge blockers", () => {
  const workflow = readVerifyWorkflow();

  assert.doesNotMatch(workflow, /react-doctor-gate:/u);
  assert.doesNotMatch(workflow, /react-doctor@0\.2\.16/u);
  assert.doesNotMatch(workflow, /--fail-on warning/u);
});

test("React Doctor keeps current debt as a narrow baseline", () => {
  const config = readReactDoctorConfig();

  assert.deepEqual(config.ignore.overrides, [
    {
      files: ["src/features/frontstage/pages/FrontStagePage.tsx"],
      rules: [
        "react-doctor/no-cascading-set-state",
        "react-doctor/no-effect-chain",
        "react-doctor/no-prop-callback-in-effect",
        "react-doctor/no-inline-exhaustive-style",
        "react-doctor/no-giant-component",
        "react-doctor/no-many-boolean-props",
        "react-doctor/prefer-useReducer",
        "react-doctor/no-derived-state-effect",
      ],
    },
    {
      files: [
        "src/features/agent-flow/_tests/editor/agent-flow-canvas-interactions.test.tsx",
      ],
      rules: ["react-doctor/no-prop-callback-in-effect"],
    },
    {
      files: ["src/features/agent-flow/_tests/node-inspector/support.tsx"],
      rules: [
        "react-doctor/no-pass-data-to-parent",
        "react-doctor/no-prop-callback-in-effect",
        "react-doctor/only-export-components",
      ],
    },
    {
      files: ["src/features/agent-flow/components/editor/AgentFlowCanvas.tsx"],
      rules: [
        "react-doctor/no-pass-data-to-parent",
        "react-doctor/no-prop-callback-in-effect",
      ],
    },
    {
      files: ["src/features/agent-flow/components/nodes/AgentFlowNodeCard.tsx"],
      rules: [
        "react-doctor/no-giant-component",
        "react-doctor/prefer-tag-over-role",
      ],
    },
    {
      files: [
        "src/features/applications/components/api/ApplicationApiKeysPanel.tsx",
      ],
      rules: ["react-doctor/prefer-module-scope-pure-function"],
    },
    {
      files: ["src/features/applications/pages/ApplicationLogsPage.tsx"],
      rules: [
        "react-doctor/no-adjust-state-on-prop-change",
        "react-doctor/no-cascading-set-state",
        "react-doctor/no-chain-state-updates",
        "react-doctor/no-derived-state-effect",
        "react-doctor/no-giant-component",
        "react-doctor/prefer-tag-over-role",
        "react-doctor/prefer-useReducer",
      ],
    },
    {
      files: ["src/features/applications/pages/ApplicationMonitoringPage.tsx"],
      rules: ["react-doctor/no-giant-component"],
    },
    {
      files: [
        "src/features/frontstage/components/FrontStagePageTreeSidebar.tsx",
      ],
      rules: [
        "react-doctor/click-events-have-key-events",
        "react-doctor/client-localstorage-no-version",
        "react-doctor/no-noninteractive-element-interactions",
        "react-doctor/no-render-in-render",
        "react-doctor/no-static-element-interactions",
      ],
    },
    {
      files: [
        "src/features/settings/components/host-infrastructure/HostInfrastructureCachePanel.tsx",
      ],
      rules: [
        "react-doctor/no-chain-state-updates",
        "react-doctor/no-giant-component",
        "react-doctor/query-mutation-missing-invalidation",
      ],
    },
    {
      files: [
        "src/features/settings/components/host-infrastructure/HostInfrastructureMemoryObservationPanel.tsx",
      ],
      rules: [
        "react-doctor/exhaustive-deps",
        "react-doctor/js-combine-iterations",
        "react-doctor/no-cascading-set-state",
        "react-doctor/no-chain-state-updates",
        "react-doctor/no-derived-state-effect",
        "react-doctor/no-giant-component",
        "react-doctor/no-static-element-interactions",
        "react-doctor/no-tiny-text",
        "react-doctor/prefer-tag-over-role",
        "react-doctor/prefer-use-effect-event",
        "react-doctor/prefer-useReducer",
        "react-doctor/query-mutation-missing-invalidation",
      ],
    },
    {
      files: [
        "src/features/settings/components/model-providers/ModelProviderInstanceDrawer.tsx",
      ],
      rules: [
        "react-doctor/no-adjust-state-on-prop-change",
        "react-doctor/no-cascading-set-state",
        "react-doctor/no-chain-state-updates",
        "react-doctor/no-derived-state",
        "react-doctor/no-event-handler",
        "react-doctor/no-giant-component",
        "react-doctor/prefer-useReducer",
        "react-doctor/rerender-state-only-in-handlers",
      ],
    },
    {
      files: ["src/shared/ui/api-docs/ApiDocsExplorer.tsx"],
      rules: [
        "react-doctor/no-adjust-state-on-prop-change",
        "react-doctor/no-derived-state",
        "react-doctor/no-event-handler",
        "react-doctor/no-giant-component",
        "react-doctor/no-pass-data-to-parent",
        "react-doctor/no-prop-callback-in-effect",
        "react-doctor/no-render-in-render",
        "react-doctor/prefer-module-scope-pure-function",
      ],
    },
    {
      files: ["src/shared/ui/api-docs/_tests/ApiDocsExplorer.test.tsx"],
      rules: ["react-doctor/anchor-is-valid"],
    },
  ]);
});

test("GitHub automation docs describe latest-only issue publishing", () => {
  const readme = readGitHubAutomationDocs();

  assert.match(readme, /push` to `latest`/u);
  assert.match(
    readme,
    /INPUT_PUBLISH_ISSUE: \$\{\{ github\.event_name == 'push' && github\.ref == 'refs\/heads\/latest' \}\}/u,
  );
  assert.match(
    readme,
    /creates a GitHub Issue only for `latest` branch pushes/u,
  );
  assert.doesNotMatch(readme, /main branch push failures/u);
  assert.doesNotMatch(readme, /refs\/heads\/main/u);
});

test("GitHub automation docs keep React Doctor in nightly and manual full gates", () => {
  const readme = readGitHubAutomationDocs();

  assert.match(readme, /React Doctor is no longer an automatic PR merge blocker/u);
  assert.match(
    readme,
    /npx react-doctor@0\.2\.16 web\/app --diff origin\/main --offline --fail-on warning --verbose/u,
  );
  assert.match(readme, /web\/app\/doctor\.config\.json/u);
  assert.match(readme, /nightly or manual full quality gate/u);
});

test("quality gate workflow supports dispatch targets and nightly latest CI defaults", () => {
  const workflow = readQualityGateWorkflow();

  assert.match(workflow, /name: quality gate/u);
  assert.match(
    workflow,
    /target_branch:\n\s+description: Target branch\n\s+type: string\n\s+default: latest/u,
  );
  assert.match(
    workflow,
    /concurrency:\n\s+group: quality-gate-\$\{\{ github\.event_name \}\}-\$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.target_branch \|\| 'latest' \}\}\n\s+cancel-in-progress: true/u,
  );
  assert.match(
    workflow,
    /schedule:\n(?:\s+# .+\n)?\s+- cron: ["']0 18 \* \* \*["']/u,
  );
  assert.match(
    workflow,
    /QUALITY_GATE_TARGET_BRANCH: \$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.target_branch \|\| 'latest' \}\}/u,
  );
  assert.match(workflow, /FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true/u);
  assert.match(
    workflow,
    /QUALITY_GATE_SCOPE: \$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.scope \|\| 'ci' \}\}/u,
  );
  assert.match(
    workflow,
    /QUALITY_GATE_REPORT_TYPE: \$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.report_type \|\| 'ci' \}\}/u,
  );
  assert.match(workflow, /QUALITY_GATE_SCHEDULED_ENVIRONMENT: nightly-latest/u);
  assert.match(workflow, /- container-images/u);
  assert.match(
    workflow,
    /GITHUB_REF_NAME: \$\{\{ env\.QUALITY_GATE_TARGET_BRANCH \}\}/u,
  );
  assert.match(
    workflow,
    /GITHUB_SHA: \$\{\{ env\.QUALITY_GATE_TARGET_SHA \}\}/u,
  );
  assert.match(
    workflow,
    /environment: \$\{\{ github\.event_name == 'schedule' && env\.QUALITY_GATE_SCHEDULED_ENVIRONMENT \|\| inputs\.environment \}\}/u,
  );
});

test("quality gate workflow runs ci scope as parallel component gates before one published aggregate report", () => {
  const workflow = readQualityGateWorkflow();

  assert.match(
    workflow,
    /repo-tooling-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(
    workflow,
    /repo-frontend-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(
    workflow,
    /repo-backend-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(workflow, /- repo-backend-static/u);
  assert.match(workflow, /- repo-backend-clippy-runtime-storage/u);
  assert.match(workflow, /- repo-backend-test-control-plane/u);
  assert.match(workflow, /- repo-backend-test-api-server/u);
  assert.match(workflow, /- repo-backend-test-plugin-runner/u);
  assert.doesNotMatch(workflow, /- repo-backend-test-apps/u);
  assert.match(workflow, /- repo-backend-check-apps/u);
  assert.match(
    workflow,
    /backend-consistency-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(
    workflow,
    /coverage-frontend-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(
    workflow,
    /coverage-backend-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\) \}\}/u,
  );
  assert.match(
    workflow,
    /container-images-gate:\n\s+if: \$\{\{ github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && \(inputs\.scope == 'ci' \|\| inputs\.scope == 'container-images'\)\) \}\}/u,
  );
  assert.match(workflow, /- coverage-backend-control-plane/u);
  assert.match(workflow, /- coverage-backend-orchestration-runtime/u);
  assert.match(workflow, /- coverage-backend-plugin-runner/u);
  assert.match(workflow, /- coverage-backend-storage-postgres/u);
  assert.match(workflow, /- coverage-backend-api-server/u);
  assert.match(
    workflow,
    /aggregate:\n\s+if: \$\{\{ always\(\) && \(github\.event_name == 'schedule' \|\| \(github\.event_name == 'workflow_dispatch' && inputs\.scope == 'ci'\)\) \}\}/u,
  );
  assert.match(
    workflow,
    /aggregate:\n(?:.*\n)*?\s+needs:\n\s+- repo-tooling-gate\n\s+- repo-frontend-gate\n\s+- repo-backend-gate\n\s+- backend-consistency-gate\n\s+- coverage-frontend-gate\n\s+- coverage-backend-gate/u,
  );
  assert.match(workflow, /- container-images-gate/u);
  assert.match(workflow, /scope: repo-tooling/u);
  assert.match(workflow, /scope: repo-frontend/u);
  assert.match(workflow, /scope: \$\{\{ matrix\.scope \}\}/u);
  assert.match(
    workflow,
    /start_postgres: \$\{\{ startsWith\(matrix\.scope, 'repo-backend-test-'\) \}\}/u,
  );
  assert.match(workflow, /scope: backend-consistency/u);
  assert.match(workflow, /scope: coverage-frontend/u);
  assert.match(workflow, /scope: container-images/u);
  assert.match(workflow, /publish_issue: "false"/u);
  assert.match(workflow, /INPUT_PUBLISH_ISSUE: "true"/u);
  assert.match(workflow, /INPUT_EXPECTED_SCOPES: .*container-images/u);
  assert.match(
    workflow,
    /node scripts\/node\/cli\/github-quality-gate-aggregate\.js/u,
  );
  assert.match(workflow, /name: test-governance-repo-tooling/u);
  assert.match(workflow, /name: test-governance-repo-frontend/u);
  assert.match(workflow, /name: test-governance-\$\{\{ matrix\.scope \}\}/u);
  assert.match(workflow, /name: test-governance-backend-consistency/u);
  assert.match(workflow, /name: test-governance-coverage-frontend/u);
  assert.match(workflow, /name: test-governance-container-images/u);
  assert.match(workflow, /name: test-governance-artifacts/u);
});

test("container image workflows keep vulnerability findings as warnings", () => {
  const publishWorkflow = readContainerImagesWorkflow();
  const qualityGateWorkflow = readQualityGateWorkflow();

  assert.match(
    publishWorkflow,
    /Enforce CRITICAL Trivy release gate[\s\S]*?output: tmp\/test-governance\/trivy-\$\{\{ matrix\.component \}\}-critical\.json\n\s+exit-code: "0"/u,
  );
  assert.match(
    publishWorkflow,
    /scope: container-images\n\s+report_type: cd\n\s+environment: container-images\n\s+publish_issue: "false"/u,
  );
  assert.match(
    qualityGateWorkflow,
    /scope: container-images\n\s+report_type: \$\{\{ env\.QUALITY_GATE_REPORT_TYPE \}\}\n\s+environment: \$\{\{ github\.event_name == 'schedule' && env\.QUALITY_GATE_SCHEDULED_ENVIRONMENT \|\| inputs\.environment \|\| 'container-images' \}\}\n\s+publish_issue: "false"/u,
  );
});

test("container image publishing avoids deprecated artifact runtime and qemu cache races", () => {
  const workflow = readContainerImagesWorkflow();
  const apiServerDockerfile = readApiServerDockerfile();

  assert.doesNotMatch(workflow, /actions\/upload-artifact@v4/u);
  assert.match(workflow, /actions\/upload-artifact@v6/u);
  assert.match(
    workflow,
    /docker\/setup-qemu-action@v4[\s\S]*?with:\n\s+cache-image: false/u,
  );
  assert.match(workflow, /promote_official_tags:/u);
  assert.match(
    workflow,
    /if: github\.event_name != 'workflow_dispatch' \|\| inputs\.promote_official_tags/u,
  );
  assert.match(
    workflow,
    /build-api-server-binary:[\s\S]*?runner: ubuntu-24\.04-arm/u,
  );
  assert.match(
    workflow,
    /docker run --rm[\s\S]*?--platform "linux\/\$\{\{ matrix\.arch \}\}"[\s\S]*?rust:1-slim-bookworm/u,
  );
  assert.match(
    workflow,
    /publish-api-server:[\s\S]*?target: runtime-prebuilt[\s\S]*?api_server_binaries=\.\/tmp\/api-server-binaries/u,
  );
  assert.match(apiServerDockerfile, /FROM runtime-base AS runtime-prebuilt/u);
  assert.match(
    apiServerDockerfile,
    /COPY --from=api_server_binaries \/\$\{TARGETARCH\}\/api-server \/usr\/local\/bin\/api-server/u,
  );
});

test("quality gate workflow keeps non-ci dispatch scopes on a single targeted job", () => {
  const workflow = readQualityGateWorkflow();

  assert.match(
    workflow,
    /single-scope-gate:\n\s+if: \$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.scope != 'ci' && inputs\.scope != 'container-images' \}\}/u,
  );
  assert.match(
    workflow,
    /single-scope-gate:[\s\S]*?fetch-depth: 0/u,
  );
  assert.match(workflow, /scope: \$\{\{ env\.QUALITY_GATE_SCOPE \}\}/u);
  assert.match(
    workflow,
    /start_postgres: \$\{\{ inputs\.scope == 'backend' \|\| inputs\.scope == 'backend-consistency' \|\| inputs\.scope == 'repo-backend' \|\| startsWith\(inputs\.scope, 'repo-backend-test-'\) \|\| inputs\.scope == 'coverage' \|\| inputs\.scope == 'coverage-backend' \|\| startsWith\(inputs\.scope, 'coverage-backend-'\) \}\}/u,
  );
  assert.match(workflow, /publish_issue: "true"/u);
});

test("quality gate action clears stale middleware containers before starting postgres", () => {
  const action = readQualityGateAction();

  assert.match(
    action,
    /cp docker\/middleware\.env\.example docker\/middleware\.env/u,
  );
  assert.match(
    action,
    /docker compose -f docker\/docker-compose\.middleware\.yaml down --remove-orphans/u,
  );
  assert.match(
    action,
    /docker-compose -f docker\/docker-compose\.middleware\.yaml down --remove-orphans/u,
  );
});

test("frontend coverage includes page-runtime package sources and tests", () => {
  const viteConfig = readWebAppViteConfig();

  assert.match(
    viteConfig,
    /\.\.\/packages\/page-runtime\/src\/_tests\/\*\*\/\*\.test\.ts/u,
  );
  assert.match(
    viteConfig,
    /include:\s*\[[\s\S]*'\.\.\/packages\/page-runtime\/src\/\*\*\/\*\.ts'/u,
  );
  assert.match(viteConfig, /allowExternal: true/u);
});
