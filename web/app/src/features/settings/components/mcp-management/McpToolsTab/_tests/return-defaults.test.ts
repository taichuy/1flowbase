import { describe, expect, it } from 'vitest';
import { parseResponseFields, toUpdateToolBody } from '../tool-editor-model';
import type { SaveConsoleMcpToolBody } from '@1flowbase/api-client';

describe('MCP return defaults', () => {
  it('keeps unset defaults distinct from an explicit empty allowlist', () => {
    expect(parseResponseFields(undefined)).toBeNull();
    expect(parseResponseFields('  ')).toBeNull();
    expect(parseResponseFields('[]')).toEqual([]);
    expect(parseResponseFields('["/items/0/body", "/a~1b/~0"]')).toEqual([
      '/items/0/body',
      '/a~1b/~0'
    ]);
  });

  it('rejects invalid paths and non-array input', () => {
    for (const value of [
      'null',
      '{}',
      '[1]',
      '["body"]',
      '["/bad~2"]',
      '["/bad~"]'
    ]) {
      expect(() => parseResponseFields(value)).toThrow();
    }
  });

  it('preserves independent return controls when building an update', () => {
    const body = {
      tool_id: 'tool',
      max_inline_chars: 80000,
      response_fields: []
    } as unknown as SaveConsoleMcpToolBody;
    expect(toUpdateToolBody(body)).toEqual({
      max_inline_chars: 80000,
      response_fields: []
    });
  });
});
