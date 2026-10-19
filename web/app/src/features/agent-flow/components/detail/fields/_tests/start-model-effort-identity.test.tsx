import { createRef, useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test } from 'vitest';
import type { FlowStartModelDescriptor } from '@1flowbase/flow-schema';
import { StartModelSettingsPanel } from '../StartModelSettingsPanel';
import { i18nText } from '../../../../../../shared/i18n/text';

function Harness() {
  const [model, setModel] = useState<FlowStartModelDescriptor>({
    id: 'model',
    reasoning: { supported_efforts: ['low', 'high'] }
  });
  return (
    <StartModelSettingsPanel
      mode="edit"
      model={model}
      triggerRef={createRef<HTMLElement>()}
      onClose={() => {}}
      onSave={() => {}}
      onChange={(patch) => setModel((current) => ({ ...current, ...patch }))}
    />
  );
}

test('deleting an earlier supported effort keeps the surviving input identity', () => {
  render(<Harness />);
  const survivor = screen.getByRole('textbox', {
    name: i18nText('agentFlow', 'auto.model_supported_reasoning_effort_input', {
      value1: 2
    })
  });
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.remove_supported_reasoning_effort', {
        value1: 1
      })
    })
  );
  expect(
    screen.getByRole('textbox', {
      name: i18nText(
        'agentFlow',
        'auto.model_supported_reasoning_effort_input',
        { value1: 1 }
      )
    })
  ).toBe(survivor);
  expect(survivor).toHaveValue('high');
});
