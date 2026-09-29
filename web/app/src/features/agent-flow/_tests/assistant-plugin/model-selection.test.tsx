import type { ReactNode } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, expect, test, vi } from 'vitest';
import {
  AssistantPanelPlugin,
  type AssistantPanelRuntime
} from '../../components/assistant-plugin/AssistantPanelPlugin';
import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';
import { i18nText } from '../../../../shared/i18n/text';

vi.mock('../../components/debug-console/AgentFlowDebugConsole', () => ({
  AgentFlowDebugConsole: ({
    composerHeader,
    composerFooterActions
  }: {
    composerHeader?: ReactNode;
    composerFooterActions?: ReactNode;
  }) => (
    <div>
      {composerHeader}
      {composerFooterActions}
    </div>
  )
}));
beforeAll(() => loadApplicationI18nResources());
const runtime: AssistantPanelRuntime = {
  preference: { model: 'removed-model', reasoning_effort: 'high' },
  run_capabilities: {
    model_selection_enabled: true,
    reasoning_effort_enabled: true,
    models: [
      {
        id: 'published-model',
        name: 'Published model',
        context_window: 1000,
        reasoning_efforts: ['low'],
        default_reasoning_effort: 'low'
      }
    ]
  },
  onChangePreference: vi.fn()
};
function panel(value: AssistantPanelRuntime) {
  return (
    <AssistantPanelPlugin
      runtime={value}
      activity={{ onClick: vi.fn() }}
      history={{ onClick: vi.fn() }}
      messages={[]}
      runContext={{
        fields: [],
        environmentLabel: 'published',
        remembered: false
      }}
      status="idle"
      stopping={false}
      onChangeRunContextValue={vi.fn()}
      onClearSession={vi.fn()}
      onClose={vi.fn()}
      onStopRun={vi.fn()}
      onSubmitPrompt={vi.fn()}
    />
  );
}
test('removed selection is explicit and never silently displays the first model', async () => {
  render(panel(runtime));
  expect(screen.getByRole('alert')).toHaveTextContent(
    i18nText('appShell', 'auto.assistant_model_reselect')
  );
  const button = screen.getByRole('button', {
    name: i18nText('appShell', 'auto.assistant_model_unavailable')
  });
  expect(button).not.toHaveTextContent('Published model');
  expect(screen.queryByText('high')).not.toBeInTheDocument();
  fireEvent.click(button);
  fireEvent.click(
    await screen.findByText(
      i18nText('appShell', 'auto.assistant_reset_defaults')
    )
  );
  expect(runtime.onChangePreference).toHaveBeenCalledWith({
    model: null,
    reasoning_effort: null
  });
});
test('selecting defaults or a valid model removes the warning', () => {
  const { rerender } = render(panel(runtime));
  rerender(
    panel({ ...runtime, preference: { model: null, reasoning_effort: null } })
  );
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: /Published model/ })
  ).toHaveTextContent('low');
});

test('each opening refreshes the runtime catalog and refreshed options replace the old menu', async () => {
  const onRefresh = vi.fn().mockResolvedValue(undefined);
  const value = {
    ...runtime,
    preference: { model: 'published-model' },
    onRefresh
  };
  const { rerender } = render(panel(value));
  const button = screen.getByRole('button', { name: /Published model/ });
  fireEvent.click(button);
  expect(onRefresh).toHaveBeenCalledTimes(1);
  const updated = {
    ...value,
    preference: { model: 'new-model' },
    run_capabilities: {
      ...value.run_capabilities,
      models: [
        {
          ...value.run_capabilities.models[0],
          id: 'new-model',
          name: 'New model'
        }
      ]
    }
  };
  rerender(panel(updated));
  const modelMenu = await screen.findByRole('menuitem', {
    name: /模型 New model|Model New model/
  });
  fireEvent.mouseEnter(modelMenu);
  expect(
    await screen.findByRole('menuitem', { name: /New model/ })
  ).toBeInTheDocument();
  expect(screen.queryByText('Published model')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /New model/ }));
  fireEvent.click(screen.getByRole('button', { name: /New model/ }));
  expect(onRefresh).toHaveBeenCalledTimes(2);
});

test('refreshing prevents selecting from a stale menu', async () => {
  const onChangePreference = vi.fn();
  render(panel({ ...runtime, refreshing: true, onChangePreference }));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole('button', { name: /模型已不可用|Model unavailable/ })
  );
  const reset = await screen.findByText(
    i18nText('appShell', 'auto.assistant_reset_defaults')
  );
  fireEvent.click(reset);
  expect(onChangePreference).not.toHaveBeenCalled();
});
