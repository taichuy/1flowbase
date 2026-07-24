import { beforeEach, describe, expect, test, vi } from 'vitest';

const { apiFetch } = vi.hoisted(() => ({ apiFetch: vi.fn() }));

vi.mock('@1flowbase/api-client', async () => {
  const actual = await vi.importActual<typeof import('@1flowbase/api-client')>(
    '@1flowbase/api-client'
  );
  return { ...actual, apiFetch };
});

import {
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

  test('dispatches canonical ctx.api requests only inside the public Auth route boundary', async () => {
    apiFetch.mockResolvedValue({ ok: true });
    await expect(dispatchPublicAuthApi('POST', '/api/public/auth/qr/start', {
      query: { locale: 'zh' }, body: { nonce: 'n-1' }
    })).resolves.toEqual({ ok: true });
    expect(apiFetch).toHaveBeenCalledWith(expect.objectContaining({
      path: '/api/public/auth/qr/start?locale=zh',
      method: 'POST',
      body: { nonce: 'n-1' }
    }));

    await expect(
      dispatchPublicAuthApi('GET', '/api/console/users', {})
    ).rejects.toThrow('forbidden API path');
    await expect(
      dispatchPublicAuthApi('GET', '/api/public/auth/%2e%2e/console/users', {})
    ).rejects.toThrow('forbidden API path');
    expect(apiFetch).toHaveBeenCalledTimes(1);
  });
});
