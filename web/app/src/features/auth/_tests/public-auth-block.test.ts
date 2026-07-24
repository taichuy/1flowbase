import { beforeEach, describe, expect, test, vi } from 'vitest';

const { apiFetch } = vi.hoisted(() => ({ apiFetch: vi.fn() }));

vi.mock('@1flowbase/api-client', async () => {
  const actual = await vi.importActual<typeof import('@1flowbase/api-client')>(
    '@1flowbase/api-client'
  );
  return { ...actual, apiFetch };
});

import {
  createPublicAuthPreviewCapabilityHandlers,
  createPublicAuthRunRequest,
  dispatchPublicAuthApi
} from '../components/public-auth-block-host';

const instance = {
  id: 'auth-password-local',
  auth_type: 'password-local',
  title: 'Password',
  description: null,
  sort_order: 0,
  public_ui_block: 'export default { main };',
  public_variables: { self_registration_enabled: true }
};

describe('public Auth Block host adapter', () => {
  beforeEach(() => apiFetch.mockReset());

  test('uses the canonical Block program, inputs, and action form values', () => {
    const request = createPublicAuthRunRequest(instance, 2, {
      type: 'action',
      primitive: 'Button',
      actionId: 'sign_up',
      formValues: { account: 'alice', password: 'change-me' }
    });

    expect(request).toMatchObject({
      requestId: 'public-auth:auth-password-local:2',
      blockId: 'public-auth:auth-password-local',
      program: {
        kind: 'source',
        source: instance.public_ui_block,
        allowedImports: [
          '@1flowbase/block-sdk',
          '@1flowbase/block-renderer/antd-facade'
        ]
      },
      inputs: {
        authenticator_id: instance.id,
        public_variables: instance.public_variables,
        auth_event: {
          action_id: 'sign_up',
          values: { account: 'alice', password: 'change-me' }
        }
      }
    });
  });

  test('dispatches canonical ctx.api requests only inside the public API boundary', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    await expect(dispatchPublicAuthApi('POST', '/api/public/auth/qr/start', {
      query: { locale: 'zh' }, body: { nonce: 'n-1' }
    })).resolves.toEqual({ ok: true });
    expect(apiFetch).toHaveBeenCalledWith(expect.objectContaining({
      path: '/api/public/auth/qr/start?locale=zh',
      method: 'POST',
      body: { nonce: 'n-1' }
    }));

    await expect(dispatchPublicAuthApi('GET', '/api/public/mapped/status', {}))
      .resolves.toEqual({ ok: true });
    expect(apiFetch).toHaveBeenLastCalledWith(expect.objectContaining({
      path: '/api/public/mapped/status',
      method: 'GET'
    }));

    await expect(
      dispatchPublicAuthApi('GET', '/api/console/users', {})
    ).rejects.toThrow('forbidden API path');
    await expect(
      dispatchPublicAuthApi('GET', '/api/public/%2e%2e/console/users', {})
    ).rejects.toThrow('forbidden API path');
    expect(apiFetch).toHaveBeenCalledTimes(2);
  });

  test('AC-033 confirms preview writes before dispatch and cancels without side effects', async () => {
    const confirmWrite = vi.fn().mockResolvedValue(false);
    const capabilities = createPublicAuthPreviewCapabilityHandlers();
    await capabilities.prepareDraftRun({
      runId: 'draft:auth-password-local:1',
      confirmWrite
    });

    await expect(
      capabilities.interface({
        type: 'interface',
        requestId: 'draft:auth-password-local:1',
        method: 'POST',
        path: '/api/public/auth/sign-up',
        request: { body: { account: 'alice' } }
      })
    ).rejects.toThrow('cancelled');

    expect(confirmWrite).toHaveBeenCalledTimes(1);
    expect(apiFetch).not.toHaveBeenCalled();
  });

  test('AC-033 dispatches preview reads without write confirmation', async () => {
    apiFetch.mockResolvedValue({ state: 'ready' });
    const confirmWrite = vi.fn().mockResolvedValue(false);
    const capabilities = createPublicAuthPreviewCapabilityHandlers();
    await capabilities.prepareDraftRun({
      runId: 'draft:auth-password-local:read',
      confirmWrite
    });

    await expect(
      capabilities.interface({
        type: 'interface',
        requestId: 'draft:auth-password-local:read',
        method: 'GET',
        path: '/api/public/auth/status'
      })
    ).resolves.toEqual({ state: 'ready' });

    expect(confirmWrite).not.toHaveBeenCalled();
    expect(apiFetch).toHaveBeenCalledTimes(1);
  });

  test('AC-034 confirms once per preview run and revokes the run capability', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    const confirmWrite = vi.fn().mockResolvedValue(true);
    const capabilities = createPublicAuthPreviewCapabilityHandlers();
    await capabilities.prepareDraftRun({
      runId: 'draft:auth-password-local:write',
      confirmWrite
    });
    const effect = {
      type: 'interface' as const,
      requestId: 'draft:auth-password-local:write',
      method: 'POST',
      path: '/api/public/auth/sign-up'
    };

    await capabilities.interface(effect);
    await capabilities.interface(effect);
    expect(confirmWrite).toHaveBeenCalledTimes(1);
    expect(apiFetch).toHaveBeenCalledTimes(2);

    capabilities.revokeDraftRun('draft:auth-password-local:write');
    await expect(capabilities.interface(effect)).rejects.toThrow(
      'not registered'
    );
    expect(apiFetch).toHaveBeenCalledTimes(2);
  });
});
