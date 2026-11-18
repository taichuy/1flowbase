import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, test, vi } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import type { FlowDocumentAnalysis } from '@1flowbase/api-client';
import { analyzeDocument } from '../../../api/orchestration';
import { useDocumentAnalysis } from '../use-document-analysis';

vi.mock('../../../api/orchestration', () => ({ analyzeDocument: vi.fn() }));
const empty: FlowDocumentAnalysis = {
  diagnostics: [],
  node_outputs: [],
  variable_catalogs: []
};
afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

function deferred() {
  let resolve!: (value: FlowDocumentAnalysis) => void;
  const promise = new Promise<FlowDocumentAnalysis>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

describe('draft analysis lifecycle', () => {
  test('debounces the complete unsaved document and hides results as soon as it changes', async () => {
    vi.useFakeTimers();
    vi.mocked(analyzeDocument).mockResolvedValue(empty);
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const { result, rerender } = renderHook(
      ({ document }) => useDocumentAnalysis('app-a', document),
      { initialProps: { document } }
    );
    expect(result.current.status).toBe('pending');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenCalledWith(
      'app-a',
      document,
      expect.any(AbortSignal),
      undefined
    );
    expect(result.current.analysis).toEqual(empty);
    const next = {
      ...document,
      meta: { ...document.meta, name: 'Unsaved edit' }
    };
    rerender({ document: next });
    expect(result.current.analysis).toBeNull();
    expect(result.current.status).toBe('pending');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenLastCalledWith(
      'app-a',
      next,
      expect.any(AbortSignal),
      undefined
    );
  });

  test('ignores late responses after document or application changes, including an A-B-A draft switch', async () => {
    vi.useFakeTimers();
    const first = deferred();
    const second = deferred();
    const third = deferred();
    vi.mocked(analyzeDocument)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(third.promise);
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const other = { ...document, meta: { ...document.meta, name: 'Other' } };
    const { result, rerender } = renderHook(
      ({ app, document }) => useDocumentAnalysis(app, document),
      { initialProps: { app: 'app-a', document } }
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    rerender({ app: 'app-a', document: other });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    rerender({ app: 'app-b', document });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    await act(async () => {
      first.resolve(empty);
      second.resolve(empty);
    });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      third.resolve({
        ...empty,
        node_outputs: [{ node_id: 'current', outputs: [] }]
      });
    });
    expect(result.current.analysis?.node_outputs[0].node_id).toBe('current');
  });

  test('does not reuse the first response when the user returns to the same draft object', async () => {
    vi.useFakeTimers();
    vi.mocked(analyzeDocument).mockResolvedValue(empty);
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const { result, rerender } = renderHook(
      ({ document }) => useDocumentAnalysis('app-a', document),
      { initialProps: { document } }
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(result.current.status).toBe('ready');
    rerender({ document: { ...document } });
    rerender({ document });
    expect(result.current.status).toBe('pending');
    expect(result.current.analysis).toBeNull();
  });

  test('requests only the selected node catalog and drops late responses when selection changes or clears', async () => {
    vi.useFakeTimers();
    const first = deferred();
    const second = deferred();
    const cleared = deferred();
    vi.mocked(analyzeDocument)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(cleared.promise);
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const initialProps: { node_id: string | null } = { node_id: 'node-start' };
    const { result, rerender } = renderHook(
      ({ node_id }: { node_id: string | null }) =>
        useDocumentAnalysis('app-a', document, node_id),
      { initialProps }
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenLastCalledWith(
      'app-a',
      document,
      expect.any(AbortSignal),
      'node-start'
    );
    rerender({ node_id: 'node-llm' });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenLastCalledWith(
      'app-a',
      document,
      expect.any(AbortSignal),
      'node-llm'
    );
    await act(async () => {
      first.resolve({
        ...empty,
        variable_catalogs: [
          { node_id: 'node-start', field_path: '', variables: [] }
        ]
      });
    });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      second.resolve({
        ...empty,
        variable_catalogs: [
          { node_id: 'node-llm', field_path: '', variables: [] }
        ]
      });
    });
    expect(
      result.current.analysis?.variable_catalogs.map(
        (catalog) => catalog.node_id
      )
    ).toEqual(['node-llm']);
    rerender({ node_id: null });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenLastCalledWith(
      'app-a',
      document,
      expect.any(AbortSignal),
      undefined
    );
    await act(async () => {
      cleared.resolve(empty);
    });
    expect(result.current.analysis?.variable_catalogs).toEqual([]);
  });

  test('removes the node scope when the selected node is deleted and ignores its in-flight catalog', async () => {
    vi.useFakeTimers();
    const deleted = deferred();
    const current = deferred();
    vi.mocked(analyzeDocument)
      .mockReturnValueOnce(deleted.promise)
      .mockReturnValueOnce(current.promise);
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const { result, rerender } = renderHook(
      ({ document }) => useDocumentAnalysis('app-a', document, 'node-llm'),
      { initialProps: { document } }
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    const next = {
      ...document,
      graph: {
        ...document.graph,
        nodes: document.graph.nodes.filter((node) => node.id !== 'node-llm')
      }
    };
    rerender({ document: next });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(analyzeDocument).toHaveBeenLastCalledWith(
      'app-a',
      next,
      expect.any(AbortSignal),
      undefined
    );
    await act(async () => {
      deleted.resolve({
        ...empty,
        variable_catalogs: [
          { node_id: 'node-llm', field_path: '', variables: [] }
        ]
      });
    });
    expect(result.current.analysis).toBeNull();
    await act(async () => {
      current.resolve(empty);
    });
    expect(result.current.analysis?.variable_catalogs).toEqual([]);
  });

  test('failed analysis exposes unavailable state without falling back to a previous catalog', async () => {
    vi.useFakeTimers();
    vi.mocked(analyzeDocument).mockRejectedValue(new Error('unavailable'));
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const { result } = renderHook(() => useDocumentAnalysis('app-a', document));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(result.current.status).toBe('error');
    expect(result.current.analysis).toBeNull();
  });
});
