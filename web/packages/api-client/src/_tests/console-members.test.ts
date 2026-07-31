import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleMember,
  deleteConsoleMember,
  disableConsoleMember,
  enableConsoleMember,
  listConsoleMembers,
  replaceConsoleMemberRoles,
  resetConsoleMemberPassword,
  updateConsoleMember,
  type CreateConsoleMemberInput,
  type UpdateConsoleMemberInput
} from '../console-members';

describe('console members client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );

  test('lists members through the canonical settings route', async () => {
    await expect(listConsoleMembers()).resolves.toMatchObject({
      path: '/api/console/settings/members'
    });
  });

  test('creates member through the canonical settings route', async () => {
    const input: CreateConsoleMemberInput = {
      account: 'member',
      email: 'member@example.com',
      phone: null,
      password: 'secret',
      name: 'Member',
      nickname: '',
      introduction: '',
      email_login_enabled: true,
      phone_login_enabled: false
    };

    await expect(
      createConsoleMember(input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: input
    });
  });

  test('updates member profile through the member patch route', async () => {
    const input: UpdateConsoleMemberInput = {
      name: 'Root Next',
      nickname: 'Captain Root',
      email: 'root-next@example.com',
      phone: '13900000000',
      introduction: 'updated root profile'
    };

    await expect(
      updateConsoleMember('member-1', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1',
      method: 'PATCH',
      csrfToken: 'csrf-123',
      body: input
    });
  });

  test('deletes member through the member delete route', async () => {
    await expect(
      deleteConsoleMember('member-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });

  test('disables member through the canonical member action route', async () => {
    await expect(
      disableConsoleMember('member-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1/disable',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('enables member through the member enable route', async () => {
    await expect(
      enableConsoleMember('member-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1/enable',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('resets member password through the canonical member action route', async () => {
    const input = { new_password: 'next-secret' };

    await expect(
      resetConsoleMemberPassword('member-1', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1/reset-password',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: input
    });
  });

  test('replaces member roles through the canonical member route', async () => {
    const input = { role_codes: ['developer'] };

    await expect(
      replaceConsoleMemberRoles('member-1', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/member-1/roles',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: input
    });
  });
});
