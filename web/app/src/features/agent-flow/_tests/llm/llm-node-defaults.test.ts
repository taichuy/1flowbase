import { describe, expect, test } from 'vitest';

import { createNodeDocument } from '../../lib/document/node-factory';

describe('LLM node defaults', () => {
  test('seeds node-level output variables from the LLM runtime contract', () => {
    const node = createNodeDocument('llm', 'node-llm-2');

    expect(node.outputs).toEqual([
      { key: 'text', title: '模型输出', valueType: 'string' },
      { key: 'usage', title: '用量', valueType: 'json' }
    ]);
  });

  test('manual LLM nodes seed only an empty system prompt message', () => {
    const node = createNodeDocument('llm', 'node-llm-2');

    expect(node.config.protocol_context).toEqual({
      kind: 'selector',
      value: ['sys', 'protocol_context']
    });
    expect(node.config.context_policy).toEqual({
      integration_context: 'enabled',
      context_selector: ['node-start', 'history']
    });
    expect(node.bindings.prompt_messages).toEqual({
      kind: 'prompt_messages',
      value: [
        {
          id: 'system-1',
          role: 'system',
          content: { kind: 'templated_text', value: '' }
        }
      ]
    });
    expect(node.bindings).not.toHaveProperty('user_prompt');
  });
});
