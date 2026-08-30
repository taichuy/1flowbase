import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { expect, test } from 'vitest';

test('AC-001 keeps Monaco out of the application bootstrap graph', async () => {
  const source = await readFile(
    path.resolve(process.cwd(), 'src/main.tsx'),
    'utf8'
  );

  expect(source).not.toContain("from './app/monaco-editor'");
  expect(source).not.toContain('initializeMonacoEditor()');
});
