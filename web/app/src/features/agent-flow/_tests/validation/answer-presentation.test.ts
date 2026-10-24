import { beforeEach, expect, test } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { createNodeDocument } from '../../lib/document/node-factory';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import {
  buildNodeDependencyMap,
  validateAnswerPresentationReferences,
  validateTerminalTopology
} from '../../lib/validation/answer-presentation';
import type { AgentFlowIssue } from '../../lib/validation/issues';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});

function presentationIssues(
  template: string,
  dependencies: Map<string, Set<string>>
) {
  const answer = createNodeDocument('answer', 'answer');
  answer.bindings.answer_template = { kind: 'templated_text', value: template };
  const nodes = ['a', 'b', 'c'].map((id) => ({
    ...createNodeDocument('code', id),
    alias: id.toUpperCase()
  }));
  const issues: AgentFlowIssue[] = [];
  validateAnswerPresentationReferences(
    issues,
    answer,
    new Map(nodes.map((node) => [node.id, node])),
    dependencies
  );
  return issues;
}

test('repeated references retain issue multiplicity, duplicate-first order and first later dependency', () => {
  const issues = presentationIssues(
    '{{a.result}} {{a.result}} {{a.result}} {{b.result}} {{c.result}}',
    new Map([
      ['a', new Set(['b', 'c'])],
      ['b', new Set()],
      ['c', new Set()]
    ])
  );
  expect(issues.map((issue) => issue.title)).toEqual([
    'Answer 输出变量重复引用',
    'Answer 输出变量重复引用',
    'Answer 展示顺序违反执行依赖',
    'Answer 展示顺序违反执行依赖',
    'Answer 展示顺序违反执行依赖'
  ]);
  expect(issues.map((issue) => issue.id)).toEqual(
    Array.from(
      { length: 5 },
      (_, index) => `answer-bindings.answer_template-${index}`
    )
  );
  for (const issue of issues.slice(2)) {
    expect(issue.message).toContain('A.result');
    expect(issue.message).toContain('B.result');
    expect(issue.message).not.toContain('C.result');
  }
});

test('a source depends on itself only when a dependency cycle returns to it', () => {
  const template = '{{a.first}} {{a.second}}';
  expect(
    presentationIssues(
      template,
      new Map([
        ['a', new Set(['b'])],
        ['b', new Set()]
      ])
    )
  ).toEqual([]);
  const issues = presentationIssues(
    template,
    new Map([
      ['a', new Set(['b'])],
      ['b', new Set(['a'])]
    ])
  );
  expect(issues).toHaveLength(1);
  expect(issues[0].title).toBe('Answer 展示顺序违反执行依赖');
  expect(issues[0].message).toContain('A.first');
  expect(issues[0].message).toContain('A.second');
});

test('dependency closure includes bindings and edges while terminal diagnostics keep document order', () => {
  const document = createDefaultAgentFlowDocument({ flowId: 'dependencies' });
  const a = createNodeDocument('code', 'a');
  const b = createNodeDocument('template_transform', 'b');
  b.bindings.template = { kind: 'selector', value: ['c', 'result'] };
  document.graph.nodes = [
    a,
    b,
    createNodeDocument('code', 'c'),
    createNodeDocument('answer', 'last'),
    createNodeDocument('answer', 'first')
  ];
  document.graph.edges = [
    createEdgeDocument({ id: 'b-a', source: 'b', target: 'a' }),
    createEdgeDocument({ id: 'first-a', source: 'first', target: 'a' }),
    createEdgeDocument({ id: 'last-a', source: 'last', target: 'a' }),
    createEdgeDocument({ id: 'last-b', source: 'last', target: 'b' })
  ];
  const dependencies = buildNodeDependencyMap(
    document,
    new Set(document.graph.nodes.map((node) => node.id))
  );
  expect(
    presentationIssues('{{a.result}} {{c.result}}', dependencies)
  ).toHaveLength(1);
  const issues: AgentFlowIssue[] = [];
  validateTerminalTopology(issues, document);
  expect(issues.map((issue) => issue.id)).toEqual([
    'last-terminal-outgoing-edge',
    'first-terminal-outgoing-edge'
  ]);
});

test('early order violations do not materialize every ancestor in a cycle', () => {
  const count = 100;
  let expansions = 0;
  class CountedDependencies extends Map<string, Set<string>> {
    override get(nodeId: string) {
      expansions += 1;
      return super.get(nodeId);
    }
  }
  const dependencies = new CountedDependencies(
    Array.from({ length: count }, (_, index) => [
      `node${index}`,
      new Set([`node${(index + 1) % count}`])
    ])
  );
  const issues = presentationIssues(
    Array.from({ length: count }, (_, index) => `{{node${index}.result}}`).join(
      ' '
    ),
    dependencies
  );
  expect(issues).toHaveLength(count - 1);
  expect(issues[0].message).toContain('node1.result');
  expect(issues.at(-1)?.message).toContain('node99.result');
  expect(expansions).toBeLessThanOrEqual(count);
});
