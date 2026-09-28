import { act, fireEvent, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { ConsoleAssistantSettings } from '@1flowbase/api-client';
import { useEmbeddedAssistantSettings } from '../../hooks/useEmbeddedAssistantSettings';

const { getSettings, saveSettings } = vi.hoisted(() => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn()
}));
vi.mock('@1flowbase/api-client', () => ({
  getConsoleAssistantSettings: getSettings,
  updateConsoleAssistantSettings: saveSettings
}));
function settings(model: string): ConsoleAssistantSettings {
  return {
    preference: {
      application_id: 'flow',
      model,
      mcp_instance_ids: [],
      enabled_client_tools: []
    },
    published_agent_flows: [],
    enabled_mcp_instances: [],
    page_reference_max_bytes: 0,
    page_reference_max_count: 0,
    page_reference_max_total_bytes: 0,
    run_capabilities: {
      model_selection_enabled: true,
      reasoning_effort_enabled: false,
      models: [
        {
          id: model,
          name: model,
          context_window: null,
          reasoning_efforts: [],
          default_reasoning_effort: null
        }
      ]
    }
  };
}
const props = {
  open: true,
  workspaceId: 'workspace',
  saving: false,
  csrfToken: 'csrf'
};
beforeEach(() => {
  getSettings.mockReset();
  saveSettings.mockReset();
  getSettings.mockResolvedValue(settings('old'));
});
describe('assistant published configuration refresh', () => {
  test('reopening loads the new published model catalog', async () => {
    const { result, rerender } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() =>
      expect(result.current.settings?.run_capabilities.models[0].id).toBe('old')
    );
    rerender({ ...props, open: false });
    getSettings.mockResolvedValue(settings('published-new'));
    rerender(props);
    await waitFor(() =>
      expect(result.current.settings?.run_capabilities.models[0].id).toBe(
        'published-new'
      )
    );
  });
  test('focus and visible-tab return refresh, while closed windows do not', async () => {
    const { result, rerender } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('old')
    );
    getSettings.mockResolvedValue(settings('focus-new'));
    fireEvent(window, new Event('focus'));
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('focus-new')
    );
    getSettings.mockResolvedValue(settings('visible-new'));
    fireEvent(document, new Event('visibilitychange'));
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('visible-new')
    );
    rerender({ ...props, open: false });
    getSettings.mockClear();
    fireEvent(window, new Event('focus'));
    expect(getSettings).not.toHaveBeenCalled();
  });
  test('a delayed refresh cannot overwrite a saved selection', async () => {
    const { result } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('old')
    );
    let resolve!: (value: ConsoleAssistantSettings) => void;
    getSettings.mockImplementation(
      () =>
        new Promise<ConsoleAssistantSettings>((done) => {
          resolve = done;
        })
    );
    fireEvent(window, new Event('focus'));
    act(() => {
      result.current.invalidateSettingsRead();
      result.current.setSettings(settings('saved'));
    });
    await act(async () => {
      const stale = settings('stale');
      stale.preference.model = 'removed';
      resolve(stale);
    });
    expect(result.current.settings?.preference.model).toBe('saved');
    expect(saveSettings).not.toHaveBeenCalled();
  });
  test('workspace changes discard settings and pending responses from the previous workspace', async () => {
    let resolve!: (value: ConsoleAssistantSettings) => void;
    getSettings.mockImplementationOnce(
      () =>
        new Promise<ConsoleAssistantSettings>((done) => {
          resolve = done;
        })
    );
    const { result, rerender } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    getSettings.mockResolvedValue(settings('other-workspace'));
    rerender({ ...props, workspaceId: 'other' });
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('other-workspace')
    );
    await act(async () => {
      resolve(settings('stale-workspace'));
    });
    expect(result.current.settings?.preference.model).toBe('other-workspace');
  });
  test('focus does not start duplicate reads or fetch while saving', async () => {
    const { result, rerender } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('old')
    );
    rerender({ ...props, saving: true });
    getSettings.mockClear();
    fireEvent(window, new Event('focus'));
    expect(getSettings).not.toHaveBeenCalled();
    rerender(props);
    let resolve!: (value: ConsoleAssistantSettings) => void;
    getSettings.mockImplementation(
      () =>
        new Promise<ConsoleAssistantSettings>((done) => {
          resolve = done;
        })
    );
    fireEvent(window, new Event('focus'));
    fireEvent(document, new Event('visibilitychange'));
    expect(getSettings).toHaveBeenCalledTimes(1);
    await act(async () => {
      resolve(settings('new'));
    });
    expect(result.current.settings?.preference.model).toBe('new');
  });
  test('failed refresh preserves configuration and a later focus retries', async () => {
    const { result } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('old')
    );
    getSettings.mockRejectedValueOnce(new Error('offline'));
    fireEvent(window, new Event('focus'));
    await waitFor(() => expect(result.current.refreshError).toBe(true));
    expect(result.current.settings?.preference.model).toBe('old');
    expect(result.current.refreshError).toBe(true);
    getSettings.mockResolvedValue(settings('recovered'));
    fireEvent(window, new Event('focus'));
    await waitFor(() =>
      expect(result.current.settings?.preference.model).toBe('recovered')
    );
  });
});

test('opening the model menu refreshes the catalog and keeps a valid saved choice', async () => {
  const { result } = renderHook(useEmbeddedAssistantSettings, {
    initialProps: props
  });
  await waitFor(() =>
    expect(result.current.settings?.preference.model).toBe('old')
  );
  const published = settings('old');
  published.run_capabilities.models.unshift(
    settings('new-first').run_capabilities.models[0]
  );
  getSettings.mockResolvedValue(published);
  await act(async () => {
    await result.current.refreshSettings();
  });
  expect(
    result.current.settings?.run_capabilities.models.map((model) => model.id)
  ).toEqual(['new-first', 'old']);
  expect(result.current.settings?.preference.model).toBe('old');
  expect(saveSettings).not.toHaveBeenCalled();
});

test('a removed saved model selects and persists the first fresh model with its default effort', async () => {
  const published = settings('first');
  published.run_capabilities.reasoning_effort_enabled = true;
  published.run_capabilities.models[0].reasoning_efforts = ['low', 'medium'];
  published.run_capabilities.models[0].default_reasoning_effort = 'medium';
  published.run_capabilities.models.push(
    settings('second').run_capabilities.models[0]
  );
  published.preference.model = 'removed';
  published.preference.reasoning_effort = 'high';
  getSettings.mockResolvedValue(published);
  const saved = {
    ...published,
    preference: {
      ...published.preference,
      model: 'first',
      reasoning_effort: 'medium'
    }
  };
  saveSettings.mockResolvedValue(saved);
  const { result } = renderHook(useEmbeddedAssistantSettings, {
    initialProps: props
  });
  await waitFor(() =>
    expect(result.current.settings?.preference.model).toBe('first')
  );
  expect(saveSettings).toHaveBeenCalledWith(saved.preference, 'csrf');
  expect(result.current.settings?.preference.reasoning_effort).toBe('medium');
});

test('an empty fresh catalog clears a removed model and effort instead of inventing a choice', async () => {
  const published = settings('removed');
  published.run_capabilities.models = [];
  published.preference.reasoning_effort = 'high';
  getSettings.mockResolvedValue(published);
  const saved = {
    ...published,
    preference: { ...published.preference, model: null, reasoning_effort: null }
  };
  saveSettings.mockResolvedValue(saved);
  const { result } = renderHook(useEmbeddedAssistantSettings, {
    initialProps: props
  });
  await waitFor(() =>
    expect(saveSettings).toHaveBeenCalledWith(saved.preference, 'csrf')
  );
  expect(result.current.settings?.preference.model).toBeNull();
});

test('failed fallback persistence keeps the refreshed catalog and retries on the next opening', async () => {
  const published = settings('new');
  published.preference.model = 'removed';
  getSettings.mockResolvedValue(published);
  saveSettings.mockRejectedValueOnce(new Error('offline'));
  const { result } = renderHook(useEmbeddedAssistantSettings, {
    initialProps: props
  });
  await waitFor(() => expect(result.current.refreshError).toBe(true));
  expect(result.current.settings?.run_capabilities.models[0].id).toBe('new');
  expect(result.current.settings?.preference.model).toBe('removed');
  saveSettings.mockResolvedValue(settings('new'));
  await act(async () => {
    await result.current.refreshSettings();
  });
  expect(result.current.settings?.preference.model).toBe('new');
  expect(result.current.refreshError).toBe(false);
});

test.each([true, false])(
  'refresh handles supported effort on an unchanged model: %s',
  async (supported) => {
    const published = settings('old');
    published.preference.reasoning_effort = 'high';
    published.run_capabilities.reasoning_effort_enabled = true;
    published.run_capabilities.models[0].reasoning_efforts = supported
      ? ['low', 'high']
      : ['low'];
    published.run_capabilities.models[0].default_reasoning_effort = 'low';
    getSettings.mockResolvedValue(published);
    const saved = {
      ...published,
      preference: { ...published.preference, reasoning_effort: 'low' }
    };
    saveSettings.mockResolvedValue(saved);
    const { result } = renderHook(useEmbeddedAssistantSettings, {
      initialProps: props
    });
    await waitFor(() => expect(result.current.refreshing).toBe(false));
    expect(result.current.settings?.preference.reasoning_effort).toBe(
      supported ? 'high' : 'low'
    );
    if (supported) expect(saveSettings).not.toHaveBeenCalled();
    else expect(saveSettings).toHaveBeenCalledWith(saved.preference, 'csrf');
  }
);

test('fallback keeps the previous reasoning habit when the new first model supports it', async () => {
  const published = settings('first');
  published.preference.model = 'removed';
  published.preference.reasoning_effort = 'high';
  published.run_capabilities.reasoning_effort_enabled = true;
  published.run_capabilities.models[0].reasoning_efforts = ['low', 'high'];
  published.run_capabilities.models[0].default_reasoning_effort = 'low';
  getSettings.mockResolvedValue(published);
  const saved = {
    ...published,
    preference: { ...published.preference, model: 'first' }
  };
  saveSettings.mockResolvedValue(saved);
  const { result } = renderHook(useEmbeddedAssistantSettings, {
    initialProps: props
  });
  await waitFor(() =>
    expect(result.current.settings?.preference.model).toBe('first')
  );
  expect(result.current.settings?.preference.reasoning_effort).toBe('high');
});
