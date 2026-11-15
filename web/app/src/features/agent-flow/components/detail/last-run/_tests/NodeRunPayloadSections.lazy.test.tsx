import { act, fireEvent, render, screen } from '@testing-library/react';
import { App } from 'antd';
import { expect, test, vi } from 'vitest';
import { NodeRunPayloadSections } from '../NodeRunPayloadSections';
import { i18nText } from '../../../../../../shared/i18n/text';

vi.mock('../runtime-debug-payload', () => ({
  RuntimeDebugPayloadBlock: ({ payload }: { payload: unknown }) => (
    <pre>{JSON.stringify(payload)}</pre>
  )
}));

test('only reads the selected payload section and retains it when reopened', async () => {
  const load = vi.fn(async (section: string) => ({
    section,
    value: 'recorded value'
  }));
  render(
    <App>
      <NodeRunPayloadSections
        inputPayload={{}}
        debugPayload={{}}
        outputPayload={{}}
        onLoadSection={load}
        processAction={<button>Trajectory</button>}
      />
    </App>
  );
  expect(load).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.input')));
  await screen.findByText(/recorded value/);
  expect(load).toHaveBeenCalledExactlyOnceWith('input_payload');
  fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.input')));
  fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.input')));
  expect(load).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByText('Trajectory'));
  expect(load).toHaveBeenCalledTimes(1);
});

test('omits the process section when disabled for run payloads', () => {
  const load = vi.fn();
  render(
    <App>
      <NodeRunPayloadSections
        inputPayload={{}}
        debugPayload={{}}
        outputPayload={{}}
        onLoadSection={load}
        includeDebugPayload={false}
      />
    </App>
  );
  expect(
    screen.getByText(i18nText('agentFlow', 'auto.input'))
  ).toBeInTheDocument();
  expect(
    screen.getByText(i18nText('agentFlow', 'auto.outputs'))
  ).toBeInTheDocument();
  expect(
    screen.queryByText(i18nText('agentFlow', 'auto.data_processing'))
  ).not.toBeInTheDocument();
  expect(load).not.toHaveBeenCalled();
});

test('refreshes an expanded running section and reads the final result before stopping', async () => {
  vi.useFakeTimers();
  try {
    const load = vi
      .fn()
      .mockResolvedValueOnce({ progress: 100 })
      .mockResolvedValueOnce({ progress: 200 })
      .mockResolvedValue({ progress: 300 });
    const props = {
      inputPayload: {},
      debugPayload: {},
      outputPayload: {},
      onLoadSection: load
    };
    const view = render(
      <App>
        <NodeRunPayloadSections {...props} refreshInterval={1500} />
      </App>
    );
    await act(async () => {
      fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.outputs')));
    });
    expect(screen.getByText(/"progress":100/)).toBeVisible();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    expect(screen.getByText(/"progress":200/)).toBeVisible();
    await act(async () => {
      view.rerender(
        <App>
          <NodeRunPayloadSections {...props} />
        </App>
      );
    });
    expect(screen.getByText(/"progress":300/)).toBeVisible();
    const count = load.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(load).toHaveBeenCalledTimes(count);
  } finally {
    vi.useRealTimers();
  }
});
