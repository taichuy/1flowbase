import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { fetchWebMcpRegistrations, invokeWebMcpTool } = vi.hoisted(() => ({
  fetchWebMcpRegistrations: vi.fn(),
  invokeWebMcpTool: vi.fn()
}));

vi.mock('../api/webmcp', () => ({
  fetchWebMcpRegistrations,
  invokeWebMcpTool
}));

import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { WebMcpRegistrationLifecycle } from '../components/WebMcpRegistrationLifecycle';
import { WEBMCP_REGISTRATIONS_CHANGED_EVENT } from '../registration-events';

describe('WebMcpRegistrationLifecycle', () => {
  const registered: Array<{
    tool: {
      name: string;
      execute: (
        input: Record<string, unknown>,
        options: { signal: AbortSignal }
      ) => Promise<unknown>;
    };
    signal: AbortSignal;
  }> = [];

  beforeEach(() => {
    registered.length = 0;
    fetchWebMcpRegistrations.mockReset();
    invokeWebMcpTool.mockReset();
    resetAuthStore();
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: vi.fn(async (tool, options) => {
          registered.push({ tool, signal: options.signal });
        })
      }
    });
    fetchWebMcpRegistrations.mockResolvedValue([
      {
        instance_id: 'browser_visible',
        tools: [
          {
            operation: 'list',
            name: 'browser_visible_mcp_list',
            title: 'Browse Browser visible',
            description: 'Browse this MCP instance.',
            input_schema: { type: 'object' },
            annotations: {
              read_only_hint: true,
              untrusted_content_hint: false
            }
          }
        ]
      }
    ]);
    invokeWebMcpTool.mockResolvedValue({ items: [] });
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
  });

  test('AC-002 AC-003 registers projected tools and invokes with current CSRF', async () => {
    const view = render(<WebMcpRegistrationLifecycle />);

    await waitFor(() => expect(registered).toHaveLength(1));
    expect(fetchWebMcpRegistrations).toHaveBeenCalledWith(
      expect.any(AbortSignal)
    );
    expect(registered[0].tool.name).toBe('browser_visible_mcp_list');

    const invocationSignal = new AbortController().signal;
    await registered[0].tool.execute(
      { path: '/' },
      { signal: invocationSignal }
    );
    expect(invokeWebMcpTool).toHaveBeenCalledWith(
      'browser_visible',
      'list',
      { path: '/' },
      'csrf-123',
      invocationSignal
    );

    view.unmount();
    expect(registered[0].signal.aborted).toBe(true);
  });

  test('AC-004 replaces registrations after a workspace switch', async () => {
    const view = render(<WebMcpRegistrationLifecycle />);
    await waitFor(() => expect(registered).toHaveLength(1));
    const firstSignal = registered[0].signal;

    await act(async () => {
      useAuthStore.getState().setAuthenticated({
        csrfToken: 'csrf-456',
        actor: {
          id: 'user-1',
          account: 'root',
          effective_display_role: 'root',
          current_workspace_id: 'workspace-2'
        },
        me: null
      });
    });

    await waitFor(() => expect(fetchWebMcpRegistrations).toHaveBeenCalledTimes(2));
    expect(firstSignal.aborted).toBe(true);
    view.unmount();
  });

  test('AC-004 refreshes registrations immediately after an instance setting changes', async () => {
    const view = render(<WebMcpRegistrationLifecycle />);
    await waitFor(() => expect(registered).toHaveLength(1));
    const firstSignal = registered[0].signal;
    fetchWebMcpRegistrations.mockResolvedValueOnce([]);

    window.dispatchEvent(new Event(WEBMCP_REGISTRATIONS_CHANGED_EVENT));

    await waitFor(() => expect(fetchWebMcpRegistrations).toHaveBeenCalledTimes(2));
    expect(firstSignal.aborted).toBe(true);
    view.unmount();
  });
});
