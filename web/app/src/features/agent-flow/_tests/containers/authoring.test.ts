import { describe, expect, test } from 'vitest';
import { containerDocument, state } from './fixtures';
import { createAgentFlowEditorStore } from '../../store/editor';
import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';
import { createNodeDocument } from '../../lib/document/node-factory';
import { insertNodeAfter } from '../../lib/document/transforms/node';
import { connectNodeFromSource } from '../../lib/document/transforms/edge';
import {
  listVisibleSelectorOptions,
  isSelectorVisible
} from '../../lib/selector-options';
import { validateContainerNode } from '../../lib/validation/containers';
import { validateAuthoringDocument } from '../../../flow-editor/authoring/validation';
import {
  getBuiltinNodeRuntimeContract,
  builtinNodeRuntimeContractTypes
} from '../../lib/node-definitions/contracts';
import type { AgentFlowIssue } from '../../lib/validate-document';

describe('container authoring contract', () => {
  test('defaults and panel paths match backend fields without a guessed deadline or common error policy', () => {
    for (const type of ['loop', 'iteration'] as const) {
      const node = createNodeDocument(type, type);
      expect(node.config.round_timeout_ms).toBeUndefined();
      expect(node.config.error_policy).toBe('fail');
      expect(node.outputs).toEqual(
        expect.arrayContaining([
          { key: 'result', title: expect.any(String), valueType: 'array' }
        ])
      );
      const schema = JSON.stringify(
        resolveAgentFlowNodeSchema(type).detail.tabs.config.blocks
      );
      expect(schema).toContain('config.round_timeout_ms');
      expect(schema).toContain('bindings.output');
      expect(schema).not.toContain('policy_group');
      expect(schema).not.toContain('entry_condition');
    }
    expect(createNodeDocument('loop', 'loop').outputs).toContainEqual({
      key: 'variables',
      title: expect.any(String),
      valueType: 'object'
    });
    expect(createNodeDocument('iteration', 'iteration').config).toEqual({
      parallelism: 1,
      flatten_output: false,
      error_policy: 'fail'
    });
    expect(
      getBuiltinNodeRuntimeContract('container_start')?.defaults.outputs
    ).toEqual([]);
    expect(builtinNodeRuntimeContractTypes).not.toContain('container_start');
  });

  test('scope follows current and enclosing graphs and only output/update bindings can read the immediate body', () => {
    const document = containerDocument();
    const paths = (id: string, field = '') =>
      listVisibleSelectorOptions(document, id, [], null, field).map((option) =>
        option.value.join('/')
      );
    expect(paths('leaf')).toEqual(
      expect.arrayContaining([
        'inner/item',
        'inner/index',
        'outer/variables/count',
        'upstream/result/items'
      ])
    );
    expect(paths('leaf')).not.toContain('outer/result');
    expect(paths('outer', 'bindings.variables')).not.toContain(
      'outer/variables/count'
    );
    expect(paths('outer', 'bindings.variables')).not.toContain('inner/result');
    expect(paths('outer', 'bindings.break_condition')).toContain(
      'outer/variables/count'
    );
    expect(paths('outer', 'bindings.break_condition')).not.toContain(
      'inner/result'
    );
    expect(paths('outer', 'bindings.variable_updates')).toContain(
      'inner/result'
    );
    expect(paths('outer', 'bindings.output')).not.toContain(
      'leaf/result/result'
    );
    expect(paths('after')).toEqual(
      expect.arrayContaining(['outer/result', 'outer/variables/count'])
    );
    expect(paths('after')).not.toContain('inner/result');
    expect(
      isSelectorVisible(
        document,
        'inner',
        ['leaf', 'result', 'result'],
        [],
        null,
        'bindings.output'
      )
    ).toBe(true);
    expect(
      isSelectorVisible(
        document,
        'inner',
        ['leaf', 'result', 'result'],
        [],
        null,
        'bindings.items'
      )
    ).toBe(false);
  });

  test('renaming loop variables atomically preserves initial/update values and rewrites scoped references', () => {
    const document = containerDocument();
    const outer = document.graph.nodes.find((node) => node.id === 'outer')!;
    outer.bindings.variable_updates = {
      kind: 'named_bindings',
      value: [
        {
          name: 'count',
          value: { kind: 'selector', selector: ['outer', 'variables', 'count'] }
        }
      ]
    };
    const leaf = document.graph.nodes.find((node) => node.id === 'leaf')!;
    leaf.bindings.named_bindings = {
      kind: 'named_bindings',
      value: [
        {
          name: 'arg',
          value: { kind: 'selector', selector: ['outer', 'variables', 'count'] }
        }
      ]
    };
    const store = createAgentFlowEditorStore(state(document));
    const adapter = () =>
      createAgentFlowNodeSchemaAdapter({
        document: store.getState().workingDocument,
        nodeId: 'outer',
        setWorkingDocument: store.getState().setWorkingDocument,
        dispatch: () => undefined
      });
    adapter().setValue('config.loop_variables', [
      { key: 'total', value_type: 'number' }
    ]);
    expect(store.getState().undoStack).toHaveLength(1);
    const saved = JSON.parse(JSON.stringify(store.getState().workingDocument));
    expect(
      saved.graph.nodes.find((node: { id: string }) => node.id === 'outer')
        .bindings.variables
    ).toEqual({
      kind: 'named_bindings',
      value: [{ name: 'total', value: { kind: 'constant', value: 0 } }]
    });
    expect(
      saved.graph.nodes.find((node: { id: string }) => node.id === 'leaf')
        .bindings.named_bindings.value[0].value.selector
    ).toEqual(['outer', 'variables', 'total']);
    adapter().setValue('bindings.break_condition', {
      kind: 'condition_group',
      value: {
        operator: 'and',
        conditions: [
          {
            kind: 'rule',
            left: ['outer', 'variables', 'total'],
            comparator: 'exists'
          }
        ]
      }
    });
    adapter().setValue('bindings.break_condition', null);
    expect(
      store
        .getState()
        .workingDocument.graph.nodes.find((node) => node.id === 'outer')!
        .bindings
    ).not.toHaveProperty('break_condition');
    adapter().setValue('config.loop_variables', []);
    expect(
      store
        .getState()
        .workingDocument.graph.nodes.find((node) => node.id === 'outer')!
        .bindings.variables
    ).toEqual({ kind: 'named_bindings', value: [] });
  });

  test('rejects global entry/terminal insertion into bodies and accepts ordinary body nodes', () => {
    const document = containerDocument();
    for (const type of [
      'start',
      'workflow_start',
      'workflow_end',
      'answer'
    ] as const) {
      expect(
        insertNodeAfter(
          document,
          'inner-start',
          createNodeDocument(type, `bad-${type}`)
        )
      ).toBe(document);
      expect(
        connectNodeFromSource(document, {
          sourceNodeId: 'inner-start',
          node: createNodeDocument(type, `bad-${type}`)
        })
      ).toBe(document);
    }
    expect(
      insertNodeAfter(
        document,
        'inner-start',
        createNodeDocument('code', 'valid')
      ).graph.nodes.some((node) => node.id === 'valid')
    ).toBe(true);
    expect(
      validateAuthoringDocument(document).filter((issue) =>
        issue.nodeId?.endsWith('-start')
      )
    ).toEqual([]);
  });

  test('reports missing deadlines, scalar inputs and mismatched constants without fabricating values', () => {
    const document = containerDocument();
    const loop = document.graph.nodes.find((node) => node.id === 'outer')!;
    const iteration = document.graph.nodes.find((node) => node.id === 'inner')!;
    const issues: AgentFlowIssue[] = [];
    delete loop.config.round_timeout_ms;
    loop.bindings.variables = {
      kind: 'named_bindings',
      value: [{ name: 'count', value: { kind: 'constant', value: 'wrong' } }]
    };
    iteration.bindings.items = {
      kind: 'selector',
      value: ['upstream', 'result', 'count']
    };
    iteration.config.parallelism = 1.5;
    validateContainerNode(issues, loop, document, [], null);
    validateContainerNode(issues, iteration, document, [], null);
    expect(issues.map((issue) => issue.fieldKey)).toEqual(
      expect.arrayContaining([
        'config.round_timeout_ms',
        'bindings.variables',
        'bindings.items',
        'config.parallelism'
      ])
    );
    expect(loop.config.round_timeout_ms).toBeUndefined();
  });
});
