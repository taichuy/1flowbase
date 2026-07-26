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
  createPublicAuthNativeBlockContextCapabilities,
  createPublicAuthInputs,
  dispatchPublicAuthApi
} from '../components/public-auth-block-host';

const instance = {
  id: 'auth-password-local',
  auth_type: 'password-local',
  title: 'Password',
  description: null,
  sort_order: 0,
  public_ui_block: 'export default function AuthBlock() { return null; }',
  public_variables: { self_registration_enabled: true }
};

describe('public Auth Block host adapter', () => {
  beforeEach(() => apiFetch.mockReset());

  test('exposes stable Native Auth inputs without legacy action rerun state', () => {
    expect(
      createPublicAuthInputs(instance.id, instance.public_variables)
    ).toEqual({
      authenticator_id: instance.id,
      public_variables: instance.public_variables
    });
  });

  test('dispatches canonical ctx.api requests only inside the public API boundary', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    await expect(
      dispatchPublicAuthApi('POST', '/api/public/auth/qr/start', {
        query: { locale: 'zh' },
        body: { nonce: 'n-1' }
      })
    ).resolves.toEqual({ ok: true });
    expect(apiFetch).toHaveBeenCalledWith(
      expect.objectContaining({
        path: '/api/public/auth/qr/start?locale=zh',
        method: 'POST',
        body: { nonce: 'n-1' }
      })
    );

    await expect(
      dispatchPublicAuthApi('GET', '/api/public/mapped/status', {})
    ).resolves.toEqual({ ok: true });
    expect(apiFetch).toHaveBeenLastCalledWith(
      expect.objectContaining({
        path: '/api/public/mapped/status',
        method: 'GET'
      })
    );

    await expect(
      dispatchPublicAuthApi('GET', '/api/console/users', {})
    ).rejects.toThrow('forbidden API path');
    await expect(
      dispatchPublicAuthApi('GET', '/api/public/%2e%2e/console/users', {})
    ).rejects.toThrow('forbidden API path');
    expect(apiFetch).toHaveBeenCalledTimes(2);
  });

  test('D4-AC-001/002 binds the shared Native ctx.api contract to the public fail-closed dispatcher', async () => {
    apiFetch.mockResolvedValue({ state: 'ready' });
    const context = createPublicAuthNativeBlockContextCapabilities({
      requestId: 'public-auth:auth-password-local:1',
      instanceEpoch: 'auth-epoch-1',
      isCurrentInstance: () => true,
      outputs: { publish: () => ({ ok: true, stale: false }) }
    });

    await expect(context.api.get('/api/public/auth/status')).resolves.toEqual({
      state: 'ready'
    });
    await expect(context.api.get('/api/console/users')).rejects.toThrow(
      'forbidden API path'
    );
    expect(apiFetch).toHaveBeenCalledTimes(1);
  });

  test('D4-AC-002/003 keeps cancelled and revoked Native preview writes away from the network', async () => {
    const preview = createPublicAuthPreviewCapabilityHandlers();
    const confirmWrite = vi.fn().mockResolvedValue(false);
    const runId = 'draft:auth-password-local:native';
    await preview.prepareDraftRun({ runId, confirmWrite });
    const context = createPublicAuthNativeBlockContextCapabilities({
      requestId: runId,
      instanceEpoch: 'auth-preview-epoch-1',
      isCurrentInstance: () => true,
      interfaceHandler: preview.interface,
      outputs: { publish: () => ({ ok: true, stale: false }) }
    });

    await expect(
      context.api.post('/api/public/auth/sign-up', {
        body: { account: 'alice' }
      })
    ).rejects.toThrow('cancelled');
    preview.revokeDraftRun(runId);
    await expect(context.api.post('/api/public/auth/sign-up')).rejects.toThrow(
      'not registered'
    );
    expect(confirmWrite).toHaveBeenCalledOnce();
    expect(apiFetch).not.toHaveBeenCalled();
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
