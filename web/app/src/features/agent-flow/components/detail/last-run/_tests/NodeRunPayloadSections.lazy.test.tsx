import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  await waitFor(() => expect(screen.getByText(/recorded value/)).toBeTruthy());
  expect(load).toHaveBeenCalledExactlyOnceWith('input_payload');
  fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.input')));
  fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.input')));
  expect(load).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByText('Trajectory'));
  expect(load).toHaveBeenCalledTimes(1);
});
