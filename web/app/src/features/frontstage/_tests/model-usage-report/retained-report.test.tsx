import * as React from 'react';
import { Activity, useState } from 'react';
import source from '../../../../../../../scripts/node/model-usage-report/block-common.jsx?raw';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

// Execute the hook shipped in the authored blocks, rather than a test copy.
const hookSource = source.slice(
  source.indexOf('function useReport('),
  source.indexOf('function Filters(')
);
const useReport = new Function(
  'React',
  `const {useEffect,useRef,useState}=React; ${hookSource}; return useReport;`
)(React);
const range = (preset: string) => ({
  preset,
  started_from: `${preset}-from`,
  started_to: `${preset}-to`
});
function deferred() {
  let resolve!: (value: unknown) => void, reject!: (error: Error) => void;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function Fixture({
  get,
  visible = true
}: {
  get: (...args: unknown[]) => Promise<unknown>;
  visible?: boolean;
}) {
  const [inputs, setInputs] = useState<Record<string, unknown>>({
    timeRange: range('first')
  });
  const ctx = {
    inputs,
    api: { get },
    outputs: {
      publish: (values: Record<string, unknown>) => {
        setInputs(values);
        return Promise.resolve({ ok: true });
      }
    }
  };
  return (
    <>
      <button
        onClick={() =>
          setInputs((old) => ({ ...old, timeRange: range('second') }))
        }
      >
        second
      </button>
      <button
        onClick={() =>
          setInputs((old) => ({ ...old, timeRange: range('third') }))
        }
      >
        third
      </button>
      <Activity mode={visible ? 'visible' : 'hidden'}>
        {[true, false, false].map((owner, index) => (
          <View key={index} ctx={ctx} owner={owner} index={index} />
        ))}
      </Activity>
    </>
  );
}
function View({ ctx, owner, index }: any) {
  const state = useReport(ctx, owner);
  return (
    <section data-testid={`view-${index}`} aria-busy={state.busy}>
      {state.report && (
        <span data-testid={`report-${index}`}>{state.report.value}</span>
      )}
      {state.error && <span>failed</span>}
      {owner && <button onClick={state.retry}>retry</button>}
    </section>
  );
}
const complete = async (entry: ReturnType<typeof deferred>, value: string) => {
  await act(async () => {
    entry.resolve({ report: { value } });
  });
};
describe('authored report single producer', () => {
  it('shares one query, keeps descendants while filtering, and hot reveal does not query again', async () => {
    const first = deferred(),
      second = deferred();
    const get = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const view = render(<Fixture get={get} />);
    expect(get).toHaveBeenCalledTimes(1);
    await complete(first, 'first result');
    const nodes = [0, 1, 2].map((i) => screen.getByTestId(`report-${i}`));
    view.rerender(<Fixture get={get} visible={false} />);
    view.rerender(<Fixture get={get} />);
    expect(get).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByText('second'));
    expect(get).toHaveBeenCalledTimes(2);
    nodes.forEach((node, i) =>
      expect(screen.getByTestId(`report-${i}`)).toBe(node)
    );
    await complete(second, 'second result');
    nodes.forEach((node) => expect(node.textContent).toBe('second result'));
  });
  it('retains completion while hidden and publishes it on reveal without a second request', async () => {
    const first = deferred();
    const get = vi.fn().mockReturnValue(first.promise);
    const view = render(<Fixture get={get} />);
    view.rerender(<Fixture get={get} visible={false} />);
    await complete(first, 'hidden completion');
    view.rerender(<Fixture get={get} />);
    await act(async () => {});
    expect(get).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText('hidden completion')).toHaveLength(3);
  });
  it('rejects superseded results and preserves the last success on failure', async () => {
    const first = deferred(),
      second = deferred(),
      third = deferred();
    const get = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(third.promise);
    render(<Fixture get={get} />);
    await complete(first, 'first result');
    fireEvent.click(screen.getByText('second'));
    fireEvent.click(screen.getByText('third'));
    await act(async () => {
      third.reject(new Error('offline'));
    });
    await complete(second, 'obsolete result');
    expect(screen.queryByText('obsolete result')).toBeNull();
    expect(screen.getAllByText('first result')).toHaveLength(3);
    expect(screen.getAllByText('failed')).toHaveLength(3);
    const retry = deferred();
    get.mockReturnValueOnce(retry.promise);
    fireEvent.click(screen.getByText('retry'));
    await complete(retry, 'recovered result');
    expect(screen.getAllByText('recovered result')).toHaveLength(3);
    expect(screen.queryByText('failed')).toBeNull();
  });
});
