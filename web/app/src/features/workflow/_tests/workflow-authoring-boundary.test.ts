import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('workflow authoring boundary', () => {
  test('AC-002/003/010 consumes neutral picker, validation, and variables', () => {
    const source = [
      'picker-options.ts',
      'validate-document.ts',
      'variables.ts'
    ]
      .map((file) =>
        fs.readFileSync(path.resolve(__dirname, '../lib', file), 'utf8')
      )
      .join('\n');

    expect(source).toContain('SHARED_EXECUTION_NODE_PICKER_TYPES');
    expect(source).toContain('validateAuthoringDocument');
    expect(source).toContain('listAuthoringVariableOptions');
    expect(source).not.toContain('agent-flow');
    expect(source).not.toContain('global-start-count');
    expect(source).not.toContain('global-answer-missing');
  });
});
