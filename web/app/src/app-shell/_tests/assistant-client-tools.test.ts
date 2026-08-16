import { QueryClient } from '@tanstack/react-query';
import { describe, expect, test, vi } from 'vitest';

import { createAssistantClientTools } from '../AssistantClientTools';
import { registerFrontstageAssistantRuntime } from '../../features/frontstage/lib/assistant-frontstage-runtime';

describe('assistant client tools', () => {
  test('AC-001 declares Frontstage capabilities only while its page runtime is mounted', () => {
    const tools = createAssistantClientTools({
      queryClient: new QueryClient(),
      refreshTargets: new Map(),
      snapshot: () => ({
        href: 'https://console.example/',
        route_id: 'home',
        page_title: 'Home',
        locale: 'en-US',
        workspace_id: 'workspace-1',
        viewport: { width: 1280, height: 720 }
      })
    });
    const listener = vi.fn();
    const unsubscribe = tools.subscribeCapabilities?.(listener);

    expect(tools.toolIds).toEqual([
      'get_client_context',
      'refresh_client_view'
    ]);
    const unregister = registerFrontstageAssistantRuntime({
      execute: vi.fn()
    });
    expect(tools.toolIds).toContain('list_page_blocks');
    expect(listener).toHaveBeenCalledOnce();

    unregister();
    expect(tools.toolIds).not.toContain('list_page_blocks');
    expect(listener).toHaveBeenCalledTimes(2);
    unsubscribe?.();
  });

  test('AC-003 returns the honest complete URL from the browser address bar', async () => {
    const tools = createAssistantClientTools({
      queryClient: new QueryClient(),
      refreshTargets: new Map(),
      snapshot: () => ({
        href: 'https://console.example/settings?token=secret&tab=models#ignored',
        route_id: 'settings',
        page_title: 'Settings',
        locale: 'zh-Hans',
        workspace_id: 'workspace-1',
        viewport: { width: 1440, height: 900 }
      })
    });

    await expect(
      tools.execute({
        call_id: 'call-1',
        name: 'get_client_context',
        arguments: {}
      })
    ).resolves.toEqual({
      is_error: false,
      result: {
        url: 'https://console.example/settings?token=secret&tab=models#ignored',
        route_id: 'settings',
        page_title: 'Settings',
        locale: 'zh-Hans',
        workspace_id: 'workspace-1',
        viewport: { width: 1440, height: 900 }
      }
    });
  });

  test('AC-004 refreshes the current page and a feature-owned semantic section', async () => {
    const queryClient = new QueryClient();
    const invalidateQueries = vi
      .spyOn(queryClient, 'invalidateQueries')
      .mockResolvedValue();
    const refreshSection = vi.fn().mockResolvedValue(undefined);
    const tools = createAssistantClientTools({
      queryClient,
      refreshTargets: new Map([
        ['application.current_section', refreshSection]
      ]),
      snapshot: () => ({
        href: 'https://console.example/',
        route_id: 'home',
        page_title: 'Home',
        locale: 'en-US',
        workspace_id: 'workspace-1',
        viewport: { width: 1280, height: 720 }
      })
    });

    await expect(
      tools.execute({
        call_id: 'call-page',
        name: 'refresh_client_view',
        arguments: { scope: 'page', target_id: 'current' }
      })
    ).resolves.toMatchObject({
      is_error: false,
      result: { status: 'refreshed', scope: 'page', target_id: 'current' }
    });
    expect(invalidateQueries).toHaveBeenCalledWith({ refetchType: 'active' });

    await expect(
      tools.execute({
        call_id: 'call-section',
        name: 'refresh_client_view',
        arguments: {
          scope: 'section',
          target_id: 'application.current_section'
        }
      })
    ).resolves.toMatchObject({
      is_error: false,
      result: {
        status: 'refreshed',
        scope: 'section',
        target_id: 'application.current_section'
      }
    });
    expect(refreshSection).toHaveBeenCalledOnce();
  });
});
