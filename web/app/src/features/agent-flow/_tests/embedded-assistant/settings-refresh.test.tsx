import { act, fireEvent, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { ConsoleAssistantSettings } from '@1flowbase/api-client';
import { useEmbeddedAssistantSettings } from '../../hooks/useEmbeddedAssistantSettings';

const { getSettings } = vi.hoisted(() => ({ getSettings: vi.fn() }));
vi.mock('@1flowbase/api-client', () => ({
  getConsoleAssistantSettings: getSettings
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
const props = { open: true, workspaceId: 'workspace', saving: false };
beforeEach(() => {
  getSettings.mockReset();
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
      resolve(settings('stale'));
    });
    expect(result.current.settings?.preference.model).toBe('saved');
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
