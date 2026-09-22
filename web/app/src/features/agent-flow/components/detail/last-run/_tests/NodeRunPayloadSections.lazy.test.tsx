import { fireEvent, render, screen } from '@testing-library/react';
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
  expect(screen.getByText(i18nText('agentFlow', 'auto.input'))).toBeInTheDocument();
  expect(screen.getByText(i18nText('agentFlow', 'auto.outputs'))).toBeInTheDocument();
  expect(
    screen.queryByText(i18nText('agentFlow', 'auto.data_processing'))
  ).not.toBeInTheDocument();
  expect(load).not.toHaveBeenCalled();
});
