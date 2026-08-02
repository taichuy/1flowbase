import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

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
});
