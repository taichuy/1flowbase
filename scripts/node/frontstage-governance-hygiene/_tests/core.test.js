const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectFrontstageGovernanceInventory,
  evaluateFrontstageGovernanceHygiene,
  main,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createFixtureRepo({ healthy = false } = {}) {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), 'oneflowbase-frontstage-governance-')
  );

  writeFile(
    repoRoot,
    'api/crates/storage-durable/postgres/migrations/20260516120000_create_frontstage_pages.sql',
    healthy
      ? `create table if not exists frontstage_pages (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  parent_id uuid references frontstage_pages(id) on delete cascade,
  kind text not null check (kind in ('group', 'page')),
  rank text not null default ''
);

create index if not exists frontstage_pages_workspace_parent_rank_idx
  on frontstage_pages (workspace_id, parent_id, rank);`
      : `create table if not exists frontstage_pages (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  parent_id uuid,
  kind text not null,
  rank text not null default ''
);`
  );

  writeFile(
    repoRoot,
    'api/crates/storage-durable/postgres/migrations/20260701120000_create_frontstage_page_visibility_rules.sql',
    healthy
      ? `create table if not exists frontstage_page_visibility_rules (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  page_id uuid references frontstage_pages(id) on delete cascade,
  role_id uuid not null references roles(id) on delete cascade
);

create unique index if not exists frontstage_page_visibility_rules_page_unique_idx
  on frontstage_page_visibility_rules (workspace_id, page_id, role_id)
  where page_id is not null;

create unique index if not exists frontstage_page_visibility_rules_root_unique_idx
  on frontstage_page_visibility_rules (workspace_id, role_id)
  where page_id is null;`
      : `create table if not exists frontstage_page_visibility_rules (
  id uuid primary key,
  workspace_id uuid not null,
  page_id uuid,
  role_id uuid
);

alter table frontstage_page_visibility_rules
  add constraint frontstage_page_visibility_rules_unique
  unique (workspace_id, page_id, role_id);`
  );

  writeFile(
    repoRoot,
    'api/crates/storage-durable/postgres/src/frontstage_repository.rs',
    `async fn get_frontstage_page(&self, workspace_id: Uuid, page_id: Uuid) -> Result<Option<Page>> {
  sqlx::query("select id from frontstage_pages where workspace_id = $1 and id = $2")
    .bind(workspace_id)
    .bind(page_id)
    .fetch_optional(self.pool())
    .await
}
`
  );

  writeFile(
    repoRoot,
    'api/crates/control-plane/src/frontstage/mod.rs',
    healthy
      ? `impl<R> FrontstagePageService<R> where R: FrontstagePageRepository {
  pub async fn get_page_detail(&self, command: GetFrontstagePageDetailCommand) -> Result<Detail> {
    let actor = self.repository.load_actor_context_for_workspace(command.actor_user_id, command.workspace_id).await?;
    self.ensure_page_visible(&actor, command.workspace_id, command.page_id).await?;
    self.repository.get_frontstage_page_detail(command.workspace_id, command.page_id).await
  }

  pub async fn save_page_content(&self, command: SaveFrontstagePageContentCommand) -> Result<Detail> {
    let actor = self.repository.load_actor_context_for_workspace(command.actor_user_id, command.workspace_id).await?;
    self.ensure_design_permission(&actor)?;
    self.repository.save_frontstage_page_content(&input).await
  }

  pub async fn get_block_code(&self, command: GetFrontstageBlockCodeCommand) -> Result<BlockCode> {
    let actor = self.repository.load_actor_context_for_workspace(command.actor_user_id, command.workspace_id).await?;
    self.ensure_page_visible(&actor, command.workspace_id, command.page_id).await?;
    self.repository.get_frontstage_block_code(command.workspace_id, command.page_id, &command.code_ref).await
  }

  pub async fn save_block_code(&self, command: SaveFrontstageBlockCodeCommand) -> Result<BlockCode> {
    let actor = self.repository.load_actor_context_for_workspace(command.actor_user_id, command.workspace_id).await?;
    self.ensure_design_permission(&actor)?;
    self.repository.save_frontstage_block_code(&input).await
  }

  async fn ensure_page_parent(&self, workspace_id: Uuid, parent_id: Option<Uuid>) -> Result<()> {
    let parent = self.repository.get_frontstage_page(workspace_id, parent_id.unwrap()).await?;
    if parent.kind != domain::FrontstagePageKind::Group { return Err(error); }
    Ok(())
  }

  async fn ensure_page_visible(&self, actor: &domain::ActorContext, workspace_id: Uuid, page_id: Uuid) -> Result<()> {
    Ok(())
  }
}`
      : `impl<R> FrontstagePageService<R> where R: FrontstagePageRepository {
  pub async fn get_page_detail(&self, command: GetFrontstagePageDetailCommand) -> Result<Detail> {
    self.repository.load_actor_context_for_workspace(command.actor_user_id, command.workspace_id).await?;
    self.repository.get_frontstage_page_detail(command.workspace_id, command.page_id).await
  }

  pub async fn save_page_content(&self, command: SaveFrontstagePageContentCommand) -> Result<Detail> {
    self.ensure_existing_page(command.workspace_id, command.page_id).await?;
    self.repository.save_frontstage_page_content(&input).await
  }

  pub async fn get_block_code(&self, command: GetFrontstageBlockCodeCommand) -> Result<BlockCode> {
    self.ensure_existing_page(command.workspace_id, command.page_id).await?;
    self.repository.get_frontstage_block_code(command.workspace_id, command.page_id, &command.code_ref).await
  }

  pub async fn save_block_code(&self, command: SaveFrontstageBlockCodeCommand) -> Result<BlockCode> {
    self.ensure_existing_page(command.workspace_id, command.page_id).await?;
    self.repository.save_frontstage_block_code(&input).await
  }
}`
  );

  writeFile(
    repoRoot,
    'api/crates/access-control/src/settings_routes.rs',
    healthy
      ? `const SETTINGS_ROUTE_SPECS: &[SettingsRouteSpec] = &[
  SettingsRouteSpec {
    route_id: "settings.docs",
    surface_key: "docs",
    path: "/settings/docs",
    label_key: "auto.api_documentation",
    visibility_permission_code: "settings_route.visible.settings.docs",
    api_scopes: DOCS_API_SCOPES,
  },
];`
      : `const SETTINGS_ROUTE_SPECS: &[SettingsRouteSpec] = &[
  SettingsRouteSpec {
    route_id: "settings.frontstage-page",
    surface_key: "frontstage-page",
    path: "/frontstage/pages/$pageId",
    label_key: "auto.frontstage_page",
    visibility_permission_code: "settings_route.visible.settings.frontstage-page",
    api_scopes: FRONTSTAGE_API_SCOPES,
  },
];`
  );

  writeFile(
    repoRoot,
    'web/app/src/features/settings/lib/settings-sections.tsx',
    healthy
      ? `export const settingsSectionDefinitions: SettingsSectionDefinition[] = [
  {
    key: 'docs',
    label_key: 'auto.api_documentation',
    to: '/settings/docs'
  }
];`
      : `export const settingsSectionDefinitions: SettingsSectionDefinition[] = [
  {
    key: 'frontstage-page',
    label_key: 'auto.frontstage_page',
    to: '/frontstage/pages/$pageId'
  }
];`
  );

  return repoRoot;
}

test('evaluateFrontstageGovernanceHygiene reports migration, service, and settings registry drift', () => {
  const repoRoot = createFixtureRepo({ healthy: false });
  const inventory = collectFrontstageGovernanceInventory({ repoRoot });
  const report = evaluateFrontstageGovernanceHygiene({ inventory });

  assert.equal(report.summary.errors, 9);
  assert.equal(report.summary.warnings, 1);

  const rules = report.findings.map((finding) => finding.rule);
  assert.ok(rules.includes('frontstage-pages-parent-fk'));
  assert.ok(rules.includes('frontstage-page-visibility-page-fk'));
  assert.ok(rules.includes('frontstage-page-visibility-role-fk'));
  assert.ok(rules.includes('frontstage-page-visibility-root-rule-unique'));
  assert.ok(rules.includes('frontstage-page-tree-cycle-static-proof'));
  assert.equal(
    rules.filter((rule) => rule === 'frontstage-page-service-visibility-gate').length,
    2
  );
  assert.ok(rules.includes('backend-settings-registry-dynamic-frontstage-page'));
  assert.ok(rules.includes('frontend-settings-registry-dynamic-frontstage-page'));
});

test('evaluateFrontstageGovernanceHygiene accepts stable fixture with only static cycle proof warning', () => {
  const repoRoot = createFixtureRepo({ healthy: true });
  const inventory = collectFrontstageGovernanceInventory({ repoRoot });
  const report = evaluateFrontstageGovernanceHygiene({ inventory });

  assert.equal(report.summary.errors, 0);
  assert.equal(report.summary.warnings, 1);
  assert.equal(report.findings[0].rule, 'frontstage-page-tree-cycle-static-proof');
});

test('main writes json and markdown reports under tmp/test-governance', async () => {
  const repoRoot = createFixtureRepo({ healthy: false });
  const stdout = [];
  const stderr = [];

  const status = await main([], {
    repoRoot,
    writeStdout(text) {
      stdout.push(text);
    },
    writeStderr(text) {
      stderr.push(text);
    },
  });

  assert.equal(status, 1);
  assert.match(stdout.join(''), /frontstage-governance-hygiene\.json/u);
  assert.match(stdout.join(''), /frontstage-governance-hygiene\.md/u);
  assert.match(stderr.join(''), /frontstage-page-service-visibility-gate/u);

  const jsonReportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'frontstage-governance-hygiene.json'
  );
  const markdownReportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'frontstage-governance-hygiene.md'
  );

  assert.equal(fs.existsSync(jsonReportPath), true);
  assert.equal(fs.existsSync(markdownReportPath), true);

  const report = JSON.parse(fs.readFileSync(jsonReportPath, 'utf8'));
  assert.equal(report.summary.errors, 9);
  assert.match(
    fs.readFileSync(markdownReportPath, 'utf8'),
    /Frontstage Governance Hygiene/u
  );
});
