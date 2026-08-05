const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = path.resolve(__dirname, '../../../..');
const QA_SKILL_ROOT = path.join(REPO_ROOT, '.agents', 'skills', 'qa-evaluation');
const AUDIT_REFERENCE_ROOT = path.join(QA_SKILL_ROOT, 'references', 'audit');

const AUDIT_REFERENCES = [
  'code-audit-model.md',
  'database-query-ephemeral.md',
  'algorithms-state-concurrency.md',
  'observability-log-pipeline.md',
  'test-asset-lifecycle.md',
  'foundation-audit-cards.md',
];

function read(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), 'utf8');
}

test('AC-001/002 code audit references are directly routable and evidence bounded', () => {
  const skill = read('.agents/skills/qa-evaluation/SKILL.md');

  for (const existingTrigger of [
    'stale or incompatible test expectation',
    'console settings registry',
    'API-scope authorization',
    'scope/error-handling acceptance',
    'i18n/multilingual key-value hygiene',
  ]) {
    assert.match(skill, new RegExp(existingTrigger, 'iu'), `existing trigger must remain: ${existingTrigger}`);
  }

  for (const reference of AUDIT_REFERENCES) {
    const absolutePath = path.join(AUDIT_REFERENCE_ROOT, reference);
    assert.equal(fs.existsSync(absolutePath), true, `${reference} must exist`);
    const source = fs.readFileSync(absolutePath, 'utf8');

    for (const heading of [
      '## Goal',
      '## Invariants',
      '## Evidence',
      '## Legal Negatives',
      '## Severity',
      '## Resource Boundary',
      '## Stop Conditions',
    ]) {
      assert.match(source, new RegExp(`^${heading}$`, 'mu'), `${reference} must include ${heading}`);
    }

    assert.match(
      skill,
      new RegExp(`references/audit/${reference.replaceAll('.', '\\.')}`, 'u'),
      `${reference} must be directly routed from SKILL.md`,
    );
  }

  assert.match(skill, /数据库|索引|query plan|ephemeral/iu);
  assert.match(skill, /算法|数据结构|状态机|并发/iu);
  assert.match(skill, /日志|旁路|可观测性|correlation/iu);
  assert.match(skill, /测试生命周期|短命测试|harness|测试资产/iu);
  assert.match(skill, /AI Gateway[\s\S]*MCP Gateway[\s\S]*Application Backend[\s\S]*(?:Native React|低代码)/u);
});

test('AC-009/010 foundation cards and subagent audit protocol keep semantic owners explicit', () => {
  const foundations = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'foundation-audit-cards.md'), 'utf8');
  const model = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'code-audit-model.md'), 'utf8');

  for (const foundation of ['AI Gateway', 'MCP Gateway', 'Application Backend', 'Native React']) {
    assert.match(foundations, new RegExp(`^### ${foundation}`, 'mu'));
  }

  assert.match(foundations, /统一(?:层|审计)[\s\S]*(?:不拥有|不复制|不建立)[\s\S]*(?:产品语义|超级 DSL|运行时)/u);
  assert.match(model, /1[^\n]*3 个只读 subagent/u);
  assert.match(model, /不得嵌套调度|不再调度 subagent/u);
  assert.match(model, /Root[\s\S]*(?:去重|交叉验证|严重级别)/u);
});

test('AC-003/008 audit cards retain their deterministic and manual-review boundaries', () => {
  const database = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'database-query-ephemeral.md'), 'utf8');
  const algorithms = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'algorithms-state-concurrency.md'), 'utf8');
  const observability = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'observability-log-pipeline.md'), 'utf8');
  const tests = fs.readFileSync(path.join(AUDIT_REFERENCE_ROOT, 'test-asset-lifecycle.md'), 'utf8');

  assert.match(database, /Static contract[\s\S]*Catalog\/capacity[\s\S]*Plan\/runtime/u);
  for (const term of [
    'NotSourceOfTruth',
    'TTLBounded',
    'CapacityBounded',
    'Observable',
    'InvalidationOwned',
    'durable fallback',
  ]) {
    assert.match(database, new RegExp(term, 'iu'));
  }

  assert.match(algorithms, /自研机制[\s\S]*成熟机制不适用[\s\S]*controlled negative/u);
  assert.match(algorithms, /same owner[\s\S]*same invariant[\s\S]*same contract family/u);
  assert.match(algorithms, /文本相似[\s\S]*(?:不能|无法|不下)/u);

  assert.match(observability, /Correlation[\s\S]*CausalError/u);
  assert.match(observability, /retention[\s\S]*live\/durable/u);
  assert.match(observability, /敏感信息泄漏[\s\S]*高增长日志无界查询/u);

  assert.match(tests, /candidate → active → consolidation_due → deprecated → removed/u);
  assert.match(tests, /replacement 已 green[\s\S]*owner 确认/u);
  assert.match(tests, /不直接删除测试、harness、组件或抽象/u);
});

test('AC-011 report template records audit evidence authenticity and legal negatives', () => {
  const reportTemplate = read('.agents/skills/qa-evaluation/references/governance/report-template.md');

  for (const field of [
    'candidate identity',
    'artifact freshness',
    'Evidence',
    'Impact',
    'Legal negative',
    'Severity',
    'Unverified',
  ]) {
    assert.match(reportTemplate, new RegExp(field, 'iu'), `report template must include ${field}`);
  }
});
