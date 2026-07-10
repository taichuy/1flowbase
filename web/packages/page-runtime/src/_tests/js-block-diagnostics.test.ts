import { describe, expect, test } from 'vitest';

import { createJsBlockDiagnostics } from '../index';

describe('JS block diagnostics', () => {
  test('maps compile runtime data and action errors to contextual diagnostics', () => {
    expect(
      createJsBlockDiagnostics(
        { pageId: 'page-1', tabId: 'tab-1', blockId: 'block-1' },
        [
          {
            code: 'import_denied',
            path: 'source.imports[0]',
            message: 'Import denied',
            sourceLocation: { line: 1, column: 1 }
          },
          { code: 'runtime_error', path: 'runtime', message: 'Runtime failed' },
          { code: 'query_denied', path: 'data.query', message: 'Query denied' },
          { code: 'action_denied', path: 'actions.invoke', message: 'Action denied' }
        ]
      )
    ).toMatchObject([
      { phase: 'compile', pageId: 'page-1', sourceLocation: { line: 1, column: 1 } },
      { phase: 'runtime', tabId: 'tab-1' },
      { phase: 'data', blockId: 'block-1' },
      { phase: 'action', code: 'action_denied' }
    ]);
  });
});
