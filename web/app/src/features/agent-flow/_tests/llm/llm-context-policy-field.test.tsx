import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { SchemaAdapter } from '../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { getBuiltinNodeRuntimeContract } from '../../lib/node-definitions/contracts';
import { LLM_CONTEXT_MESSAGES_JSON_SCHEMA } from '../../lib/output-contract/schema';
import type { FlowSelectorOption } from '../../lib/selector-options';
import { agentFlowFieldRenderers } from '../../schema/agent-flow-field-renderers';

const contextOptions: FlowSelectorOption[] = [
  {
    nodeId: 'node-start',
    nodeLabel: 'Start',
    outputKey: 'history',
    outputLabel: 'history',
    valueType: 'array',
    jsonSchema: LLM_CONTEXT_MESSAGES_JSON_SCHEMA,
    value: ['node-start', 'history'],
    displayLabel: 'Start history'
  },
  {
    nodeId: 'node-code',
    nodeLabel: 'Code',
    outputKey: 'history',
    outputLabel: 'history',
    valueType: 'array',
    jsonSchema: LLM_CONTEXT_MESSAGES_JSON_SCHEMA,
    value: ['node-code', 'history'],
    displayLabel: 'Code history'
  }
];

function ContextPolicyHarness({
  initialValue
}: {
  initialValue: Record<string, unknown>;
}) {
  const [value, setValue] = useState(initialValue);
  const adapter: SchemaAdapter = {
    getValue: (path) =>
      path === 'config.context_policy.auto_compact' ? value.auto_compact : value,
    setValue: (path, nextValue) => {
      if (path === 'config.context_policy.auto_compact') {
        setValue((current) => ({ ...current, auto_compact: nextValue }));
      } else {
        setValue(nextValue as Record<string, unknown>);
      }
    },
    getDerived: (path) => (path === 'selectorOptions' ? contextOptions : null),
    dispatch: vi.fn()
  };
  const contract = getBuiltinNodeRuntimeContract('llm');
  const field = contract?.panel.sections
    .flatMap((section) => section.fields ?? [])
    .find((entry) => entry.key === 'config.context_policy.auto_compact');

  if (!field || field.renderer !== 'switch') {
    throw new Error('expected the LLM automatic compaction switch contract');
  }

  const ContextField = agentFlowFieldRenderers.llm_context_policy;
  const AutoCompactField = agentFlowFieldRenderers.switch;

  return (
    <>
      <ContextField
        adapter={adapter}
        block={{
          kind: 'field',
          label: '上下文',
          path: 'config.context_policy',
          renderer: 'llm_context_policy'
        }}
      />
      <AutoCompactField
        adapter={adapter}
        block={{
          kind: 'field',
          label: field.title,
          path: field.key,
          renderer: field.renderer
        }}
      />
      <output data-testid="context-policy-value">{JSON.stringify(value)}</output>
    </>
  );
}

function currentPolicy() {
  return JSON.parse(screen.getByTestId('context-policy-value').textContent ?? '{}');
}

describe('LLM context policy fields', () => {
  test('compaction defaults off and each switch preserves the other history settings', () => {
    render(
      <ContextPolicyHarness
        initialValue={{
          integration_context: 'enabled',
          context_selector: ['node-code', 'history']
        }}
      />
    );

    const autoCompact = screen.getByRole('switch', { name: '自动压缩上下文' });
    const integrationContext = screen.getByRole('switch', { name: '上下文' });
    expect(autoCompact).not.toBeChecked();

    fireEvent.click(autoCompact);
    expect(currentPolicy()).toEqual({
      integration_context: 'enabled',
      context_selector: ['node-code', 'history'],
      auto_compact: true
    });

    fireEvent.click(integrationContext);
    expect(currentPolicy()).toEqual({
      integration_context: 'disabled',
      context_selector: ['node-code', 'history'],
      auto_compact: true
    });
    expect(screen.getByRole('combobox', { name: '上下文变量' })).toBeDisabled();

    fireEvent.click(autoCompact);
    expect(currentPolicy()).toEqual({
      integration_context: 'disabled',
      context_selector: ['node-code', 'history'],
      auto_compact: false
    });
  });

  test('an explicitly enabled policy stays enabled when selecting different history', async () => {
    render(
      <ContextPolicyHarness
        initialValue={{
          integration_context: 'enabled',
          context_selector: ['node-start', 'history'],
          auto_compact: true
        }}
      />
    );

    expect(screen.getByRole('switch', { name: '自动压缩上下文' })).toBeChecked();
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '上下文变量' }));
    fireEvent.click(await screen.findByText('Code history'));

    expect(currentPolicy()).toEqual({
      integration_context: 'enabled',
      context_selector: ['node-code', 'history'],
      auto_compact: true
    });
    expect(screen.getByRole('switch', { name: '自动压缩上下文' })).toBeChecked();
  });
});
