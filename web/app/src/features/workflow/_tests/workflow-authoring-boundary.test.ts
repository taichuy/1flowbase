import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';
import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import '../register';
import { createNodeDocument } from '../../agent-flow/lib/document/node-factory';
import { duplicateNodeSubgraph } from '../../agent-flow/lib/document/transforms/duplicate';
import { buildNodePickerOptions } from '../../flow-editor';
import { createBuiltinCatalogNode } from '../../agent-flow/_tests/fixtures/application-node-catalog';
import { validateWorkflowDocument } from '../lib/validate-document';
import { listWorkflowVariableOptions } from '../lib/variables';

describe('workflow authoring boundary', () => {
  test('AC-004 consumes the unified server node catalog without a local picker inventory', () => {
    const source = [
      '../pages/WorkflowEditorPage.tsx',
      '../components/WorkflowCanvasFrame.tsx',
      '../lib/validate-document.ts',
      '../lib/variables.ts'
    ]
      .map((file) => fs.readFileSync(path.resolve(__dirname, file), 'utf8'))
      .join('\n');

    expect(source).toContain('fetchApplicationNodeCatalog');
    expect(source).toContain('buildNodePickerOptions(nodeCatalog.nodes)');
    expect(source).not.toContain('SHARED_EXECUTION_NODE_PICKER_TYPES');
    expect(source).toContain('validateAuthoringDocument');
    expect(source).toContain('listAuthoringVariableOptions');
    expect(source).not.toContain('global-start-count');
    expect(source).not.toContain('global-answer-missing');
  });

  test('AC-014/015 consumes Variable Aggregator group outputs through the shared Workflow authoring boundary', () => {
    const [option] = buildNodePickerOptions([
      createBuiltinCatalogNode('variable_aggregator', {
        title: 'Variable Aggregator',
        category: 'control'
      })
    ]);
    const aggregator = createNodeDocument(
      option,
      'node-variable-aggregator',
      280,
      220
    );
    aggregator.bindings.groups = {
      kind: 'variable_groups',
      value: [
        {
          key: 'group1',
          valueType: 'string',
          candidates: [
            ['node-workflow-start', 'primary'],
            ['node-workflow-start', 'fallback']
          ]
        }
      ]
    };
    aggregator.outputs = [
      { key: 'group1', title: 'group1', valueType: 'string' }
    ];

    const document = createDefaultWorkflowDocument({
      flowId: 'workflow-variable-aggregator'
    });
    document.graph.nodes.push(aggregator);
    document.graph.edges = [
      {
        id: 'edge-workflow-start-aggregator',
        source: 'node-workflow-start',
        target: aggregator.id,
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      },
      {
        id: 'edge-aggregator-workflow-end',
        source: aggregator.id,
        target: 'node-workflow-end',
        sourceHandle: null,
        targetHandle: null,
        containerId: null,
        points: []
      }
    ];

    const duplicated = duplicateNodeSubgraph(document, {
      nodeId: aggregator.id
    });
    const copy = duplicated.graph.nodes.find(
      (node) => node.id === 'node-variable-aggregator-copy'
    );

    expect(aggregator.type).toBe('variable_aggregator');
    expect(aggregator.outputs.map((output) => output.key)).toEqual(['group1']);
    expect(copy?.bindings.groups).toEqual(aggregator.bindings.groups);
    expect(
      listWorkflowVariableOptions(document, 'node-workflow-end', null).some(
        (option) =>
          option.value.join('.') === 'node-variable-aggregator.group1' &&
          option.valueType === 'string'
      )
    ).toBe(true);
    expect(
      validateWorkflowDocument(document).filter(
        (issue) => issue.nodeId === aggregator.id
      )
    ).toEqual([]);

    const workflowSources = [
      '../register.ts',
      '../lib/node-definitions.ts',
      '../components/WorkflowCanvasFrame.tsx'
    ]
      .map((file) => fs.readFileSync(path.resolve(__dirname, file), 'utf8'))
      .join('\n');

    expect(workflowSources).not.toContain('createVariableAggregatorContract');
    expect(workflowSources).not.toContain('VariableAggregatorField');
  });
});
