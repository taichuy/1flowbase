import { render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { CanvasNodeSchema } from '../contracts/canvas-node-schema';
import type { PageBlockSchema } from '../contracts/page-block-schema';
import { createRendererRegistry } from '../registry/create-renderer-registry';
import { evaluateSchemaRule } from '../runtime/rule-evaluator';
import { SchemaRenderer } from '../runtime/SchemaRenderer';

const view = createRendererRegistry({
  fields: {
    text: ({ block, adapter }) => (
      <input aria-label={block.label} value={String(adapter.getValue(block.path) ?? '')} readOnly />
    )
  },
  views: {
    summary: ({ block }) => <div>{block.title}</div>
  },
  dynamicForms: {
    llm_parameters: ({ block }) => <div>{block.form_key}</div>
  },
  shells: {}
});

const schema: CanvasNodeSchema = {
  schemaVersion: '1.0.0',
  nodeType: 'llm',
  capabilities: ['help', 'run'],
  card: {
    blocks: []
  },
  detail: {
    header: { blocks: [] },
    tabs: {
      config: {
        blocks: [
          { kind: 'view', renderer: 'summary', title: '节点说明' },
          {
            kind: 'section',
            title: 'Inputs',
            blocks: [{ kind: 'field', renderer: 'text', path: 'config.model', label: '模型' }]
          }
        ]
      },
      lastRun: { blocks: [] }
    }
  },
  runtimeSlots: {}
};

const pageBlockSchema: PageBlockSchema = {
  schemaVersion: '1.0.0',
  namespace: 'agent-flow',
  blockType: 'page_block'
};

describe('schema runtime', () => {
  test('keeps the page block contract as a reserved namespace anchor', () => {
    expect(pageBlockSchema.namespace).toBe('agent-flow');
    expect(pageBlockSchema.blockType).toBe('page_block');
  });

  test('evaluates visibility rules with capability lookups', () => {
    expect(
      evaluateSchemaRule(
        { operator: 'hasCapability', capability: 'run' },
        { capabilities: ['help', 'run'], values: {} }
      )
    ).toBe(true);
  });

  test('renders nested blocks through the registry', () => {
    render(
      <SchemaRenderer
        adapter={{
          getValue: (path: string) => (path === 'config.model' ? 'gpt-4o-mini' : null),
          setValue: vi.fn(),
          getDerived: () => null,
          dispatch: vi.fn()
        }}
        blocks={schema.detail.tabs.config.blocks}
        registry={view}
      />
    );

    expect(screen.getByText('节点说明')).toBeInTheDocument();
    expect(screen.getByLabelText('模型')).toHaveValue('gpt-4o-mini');
  });

  test('uses adapter root values when evaluating block visibility', () => {
    render(
      <SchemaRenderer
        adapter={{
          getValue: (path: string) => (path === 'config.model' ? 'gpt-4o-mini' : null),
          setValue: vi.fn(),
          getDerived: (key: string) =>
            key === 'rootValues' ? { config: { mode: 'advanced' } } : null,
          dispatch: vi.fn()
        }}
        blocks={[
          {
            kind: 'field',
            renderer: 'text',
            path: 'config.model',
            label: '模型',
            visibleWhen: { operator: 'eq', path: 'config.mode', value: 'advanced' }
          }
        ]}
        registry={view}
      />
    );

    expect(screen.getByLabelText('模型')).toHaveValue('gpt-4o-mini');
  });

  test('renders dynamic form blocks through the registry', () => {
    render(
      <SchemaRenderer
        adapter={{
          getValue: () => null,
          setValue: vi.fn(),
          getDerived: () => null,
          dispatch: vi.fn()
        }}
        blocks={[{ kind: 'dynamic_form', form_key: 'llm_parameters', title: 'LLM 参数' }]}
        registry={view}
      />
    );

    expect(screen.getByText('llm_parameters')).toBeInTheDocument();
  });
});
