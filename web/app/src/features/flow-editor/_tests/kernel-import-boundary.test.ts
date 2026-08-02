import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

const kernelRoot = path.resolve(__dirname, '..');

function readSourceFiles(directory: string): string[] {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      return entry.name === '_tests' ? [] : readSourceFiles(entryPath);
    }

    return /\.(ts|tsx)$/.test(entry.name)
      ? [fs.readFileSync(entryPath, 'utf8')]
      : [];
  });
}

describe('flow editor kernel import boundary', () => {
  test('AC-002 keeps application-specific behavior outside the kernel', () => {
    const source = readSourceFiles(kernelRoot).join('\n');

    expect(source).not.toMatch(/features\/agent-flow|\.\.\/agent-flow/);
    expect(source).not.toMatch(/features\/workflow|\.\.\/workflow/);
    expect(source).not.toContain('useAgentFlowDebugSession');
    expect(source).not.toContain('ConversationVariablesPanel');
    expect(source).not.toContain('SystemVariablesPanel');
    expect(source).not.toContain('WorkflowTriggerConfigField');
    expect(source).not.toContain('isWorkflow');
    expect(source).not.toContain('capabilities');
  });

  test('AC-004 maps server catalog entries without owning an availability list', () => {
    const source = readSourceFiles(path.join(kernelRoot, 'authoring')).join(
      '\n'
    );

    expect(source).toContain('ConsoleApplicationNodeCatalogEntry');
    expect(source).toContain('buildNodePickerOptions');
    expect(source).not.toContain('SHARED_EXECUTION_NODE_PICKER_TYPES');
    expect(source).toContain('registerNodeRuntimeContract');
    expect(source).toContain('validateAuthoringDocument');
    expect(source).toContain('normalizeStartInputField');
  });
});
