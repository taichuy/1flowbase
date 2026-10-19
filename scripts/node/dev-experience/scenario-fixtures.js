

function findFrontstageFixture(nodes, inheritedSlug = null) {
  for (const node of nodes) {
    const slug =
      typeof node.slug === "string" && node.slug.length > 0
        ? node.slug
        : inheritedSlug;
    if (node.kind === "page" && slug) return { pageId: node.id, slug };
    const nested = findFrontstageFixture(
      Array.isArray(node.children) ? node.children : [],
      slug,
    );
    if (nested) return nested;
  }
  return null;
}

async function requestData(requestContext, apiPath) {
  const response = await requestContext.get(apiPath);
  if (!response.ok()) {
    throw new Error(
      `Dev experience fixture request failed: GET ${apiPath} -> ${response.status()} ${await response.text()}`,
    );
  }
  const payload = await response.json();
  return payload?.data;
}

async function resolveScenarioFixtures({
  playwright,
  apiBaseUrl,
  storageStatePath,
  scenarios,
}) {
  const requestContext = await playwright.request.newContext({
    baseURL: apiBaseUrl,
    storageState: storageStatePath,
  });
  try {
    let pages = null;
    let applications = null;
    const resolved = [];
    for (const scenario of scenarios) {
      if (scenario.fixture_kind === "static") {
        resolved.push({ ...scenario, fixture_path: scenario.fixture_path });
        continue;
      }
      if (scenario.fixture_kind === "frontstage_page") {
        pages ||=
          (await requestData(
            requestContext,
            "/api/console/frontstage/pages",
          )) || [];
        const fixture = findFrontstageFixture(pages);
        if (!fixture) {
          throw new Error(
            "Dev experience requires at least one existing Frontstage page fixture",
          );
        }
        resolved.push({
          ...scenario,
          fixture_path: scenario.path_template
            .replace(":slug", fixture.slug)
            .replace(":page_id", fixture.pageId),
        });
        continue;
      }
      if (scenario.fixture_kind === "workflow_application") {
        applications ||=
          (await requestData(requestContext, "/api/console/applications")) ||
          [];
        const application = applications.find(
          (candidate) => candidate.application_type === "workflow",
        );
        if (!application) {
          throw new Error(
            "Dev experience requires at least one existing workflow application fixture",
          );
        }
        resolved.push({
          ...scenario,
          fixture_path: scenario.path_template.replace(
            ":application_id",
            application.id,
          ),
        });
        continue;
      }
      throw new Error(
        `Unknown dev experience fixture kind: ${scenario.fixture_kind}`,
      );
    }
    return resolved;
  } finally {
    await requestContext.dispose();
  }
}

module.exports = { resolveScenarioFixtures };
