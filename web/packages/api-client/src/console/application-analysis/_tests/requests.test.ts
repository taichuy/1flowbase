import { describe, expect, test, vi } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { analyzeConsoleApplicationDocument } from '..';
import * as transport from '../../../transport';

describe('draft analysis client', () => {
  test('posts the complete unsaved document through the read-only analysis route', async () => {
    const response = {
      diagnostics: [],
      node_outputs: [],
      variable_catalogs: []
    };
    const request = vi.spyOn(transport, 'apiFetch').mockResolvedValue(response);
    const document = createDefaultAgentFlowDocument({ flowId: 'draft-flow' });
    document.meta.name = 'Unsaved draft';
    const controller = new AbortController();
    expect(
      await analyzeConsoleApplicationDocument(
        'app-1',
        { document },
        'https://example.test',
        controller.signal
      )
    ).toBe(response);
    expect(request).toHaveBeenCalledWith({
      path: '/api/console/applications/app-1/orchestration/analysis',
      method: 'POST',
      body: { document },
      baseUrl: 'https://example.test',
      signal: controller.signal
    });
    expect(request.mock.calls[0][0].csrfToken).toBeUndefined();
    request.mockRestore();
  });
  test('requests a catalog only for the specified node without changing the full draft', async () => {
    const response = {
      diagnostics: [],
      node_outputs: [],
      variable_catalogs: [
        { node_id: 'node-llm', field_path: '', variables: [] }
      ]
    };
    const request = vi.spyOn(transport, 'apiFetch').mockResolvedValue(response);
    const document = createDefaultAgentFlowDocument({ flowId: 'draft-flow' });
    expect(
      await analyzeConsoleApplicationDocument('app-1', {
        document,
        node_id: 'node-llm'
      })
    ).toBe(response);
    expect(request).toHaveBeenCalledWith({
      path: '/api/console/applications/app-1/orchestration/analysis',
      method: 'POST',
      body: { document, node_id: 'node-llm' },
      baseUrl: undefined,
      signal: undefined
    });
    request.mockRestore();
  });
});
