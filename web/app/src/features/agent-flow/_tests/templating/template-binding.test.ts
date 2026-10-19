import { describe, expect, test } from 'vitest';

import {
  createTemplateSelectorToken,
  isTemplateSelectorToken,
  parseTemplateSelectorTokens,
  remapTemplateSelectorTokens
} from '../../lib/template-binding';

describe('template binding selectors', () => {
  test('supports nested selector paths in template tokens', () => {
    expect(
      createTemplateSelectorToken([
        'node-code',
        'result',
        'chat_history'
      ])
    ).toBe('{{node-code.result.chat_history}}');
    expect(
      parseTemplateSelectorTokens(
        'Use {{ node-code.result.chat_history }} and {{ node-start.query }}'
      )
    ).toEqual([
      ['node-code', 'result', 'chat_history'],
      ['node-start', 'query']
    ]);
  });

  test('remaps node ids while preserving nested selector tails', () => {
    expect(
      remapTemplateSelectorTokens(
        '{{ node-code.result.chat_history }}',
        new Map([['node-code', 'node-code-copy']])
      )
    ).toBe('{{node-code-copy.result.chat_history}}');
  });

  test('parses template selectors after checking a token match', () => {
    expect(isTemplateSelectorToken('{{node-start.query}}')).toBe(true);
    expect(parseTemplateSelectorTokens('Use {{node-start.query}}')).toEqual([
      ['node-start', 'query']
    ]);
  });
});
