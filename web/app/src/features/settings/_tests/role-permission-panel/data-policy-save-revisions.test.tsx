import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { resetAuthStore } from '../../../../state/auth-store';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import {
  settingsRoleDataPolicyQueryKey,
  type SettingsRoleDataPolicy,
  type ReplaceSettingsRoleDataPolicyInput
} from '../../api/identity-access/roles';
import { authenticate, defaultDataModels, defaultDataPolicy } from './fixtures';

const api = vi.hoisted(() => ({
  fetchSettingsRoleDataPolicy: vi.fn(),
  replaceSettingsRoleDataPolicy: vi.fn(),
  fetchSettingsAllDataModels: vi.fn()
}));
vi.mock('../../api/identity-access/roles', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/identity-access/roles')>()),
  fetchSettingsRoleDataPolicy: api.fetchSettingsRoleDataPolicy,
  replaceSettingsRoleDataPolicy: api.replaceSettingsRoleDataPolicy
}));
vi.mock('../../api/data-models', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/data-models')>()),
  fetchSettingsAllDataModels: api.fetchSettingsAllDataModels
}));
let RoleDataPolicySection: typeof import('../../components/role-permissions/RoleDataPolicySection').RoleDataPolicySection;
let serverPolicy: SettingsRoleDataPolicy;
let mounted: { unmount(): void; client: QueryClient } | undefined;
const sections = ['default-policy', 'single-model-policy'] as const;
type Section = (typeof sections)[number];
const createName = (section: Section) =>
  section === 'default-policy' ? '新增 启用' : '新增 Orders';
function policyWithCreate(section: Section, enabled: boolean) {
  return section === 'default-policy'
    ? {
        ...serverPolicy,
        default_policy: { ...serverPolicy.default_policy, can_create: enabled }
      }
    : {
        ...serverPolicy,
        model_policies: serverPolicy.model_policies.map((policy) =>
          policy.data_model_id === 'model-orders'
            ? { ...policy, can_create_override: enabled }
            : policy
        )
      };
}
function setup(section: Section) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const view = render(
    <QueryClientProvider client={client}>
      <RoleDataPolicySection
        roleCode="member"
        canEdit
        section={section}
        formId="save-revision-policy"
      />
      <button type="submit" form="save-revision-policy">
        Save policy
      </button>
    </QueryClientProvider>
  );
  mounted = { unmount: view.unmount, client };
  return client;
}
async function finishSave(client: QueryClient) {
  await waitFor(() =>
    expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalled()
  );
  await waitFor(() => expect(client.isMutating()).toBe(0));
}
async function refresh(client: QueryClient, policy: SettingsRoleDataPolicy) {
  serverPolicy = policy;
  await act(async () => {
    client.setQueryData(settingsRoleDataPolicyQueryKey('member'), policy);
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });
}
beforeEach(async () => {
  vi.clearAllMocks();
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
  ({ RoleDataPolicySection } =
    await import('../../components/role-permissions/RoleDataPolicySection'));
  resetAuthStore();
  authenticate();
  serverPolicy = structuredClone(defaultDataPolicy) as SettingsRoleDataPolicy;
  api.fetchSettingsRoleDataPolicy.mockImplementation(async () => serverPolicy);
  api.fetchSettingsAllDataModels.mockImplementation(async () =>
    defaultDataModels()
  );
  api.replaceSettingsRoleDataPolicy.mockImplementation(
    async (_role: string, input: ReplaceSettingsRoleDataPolicyInput) => {
      serverPolicy = { ...serverPolicy, ...input };
    }
  );
});
afterEach(() => {
  mounted?.unmount();
  mounted?.client.clear();
  mounted = undefined;
  resetAuthStore();
});

test.each(sections)(
  '%s saved edits relinquish ownership to later server refreshes',
  async (section) => {
    const client = setup(section);
    const create = await screen.findByRole('checkbox', {
      name: createName(section)
    });
    await waitFor(() => expect(create).toBeEnabled());
    fireEvent.click(create);
    expect(create).not.toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
    await finishSave(client);
    await refresh(client, policyWithCreate(section, true));
    expect(
      screen.getByRole('checkbox', { name: createName(section) })
    ).toBeChecked();
  }
);

test.each(sections)(
  '%s preserves edits made during a slow successful request',
  async (section) => {
    let complete!: () => void;
    const pending = new Promise<void>((resolve) => {
      complete = resolve;
    });
    api.replaceSettingsRoleDataPolicy.mockImplementationOnce(
      async (_role: string, input: ReplaceSettingsRoleDataPolicyInput) => {
        await pending;
        serverPolicy = { ...serverPolicy, ...input };
      }
    );
    const client = setup(section);
    const create = await screen.findByRole('checkbox', {
      name: createName(section)
    });
    await waitFor(() => expect(create).toBeEnabled());
    fireEvent.click(create);
    fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
    await waitFor(() =>
      expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalled()
    );
    fireEvent.click(create);
    await act(async () => {
      complete();
      await pending;
    });
    await finishSave(client);
    expect(
      screen.getByRole('checkbox', { name: createName(section) })
    ).toBeChecked();
    await refresh(client, policyWithCreate(section, false));
    expect(
      screen.getByRole('checkbox', { name: createName(section) })
    ).toBeChecked();
  }
);

test.each(sections)(
  '%s retains a newer revision even when it returns to the submitted value',
  async (section) => {
    let complete!: () => void;
    const pending = new Promise<void>((resolve) => {
      complete = resolve;
    });
    api.replaceSettingsRoleDataPolicy.mockImplementationOnce(
      async (_role: string, input: ReplaceSettingsRoleDataPolicyInput) => {
        await pending;
        serverPolicy = { ...serverPolicy, ...input };
      }
    );
    const client = setup(section);
    const create = await screen.findByRole('checkbox', {
      name: createName(section)
    });
    await waitFor(() => expect(create).toBeEnabled());
    fireEvent.click(create);
    fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
    await waitFor(() =>
      expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalled()
    );
    fireEvent.click(create);
    fireEvent.click(create);
    await act(async () => {
      complete();
      await pending;
    });
    await finishSave(client);
    await refresh(client, policyWithCreate(section, true));
    expect(
      screen.getByRole('checkbox', { name: createName(section) })
    ).not.toBeChecked();
  }
);

test('retires submitted default fields while preserving a different field edited during the save', async () => {
  let complete!: () => void;
  const pending = new Promise<void>((resolve) => {
    complete = resolve;
  });
  api.replaceSettingsRoleDataPolicy.mockImplementationOnce(
    async (_role: string, input: ReplaceSettingsRoleDataPolicyInput) => {
      await pending;
      serverPolicy = { ...serverPolicy, ...input };
    }
  );
  const client = setup('default-policy');
  const create = await screen.findByRole('checkbox', { name: '新增 启用' });
  await waitFor(() => expect(create).toBeEnabled());
  fireEvent.click(create);
  fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
  await waitFor(() =>
    expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalled()
  );
  fireEvent.click(screen.getByRole('checkbox', { name: '查看 启用' }));
  await act(async () => {
    complete();
    await pending;
  });
  await finishSave(client);
  await refresh(client, {
    ...policyWithCreate('default-policy', true),
    default_policy: {
      ...serverPolicy.default_policy,
      can_create: true,
      can_view: true
    }
  });
  expect(screen.getByRole('checkbox', { name: '新增 启用' })).toBeChecked();
  expect(screen.getByRole('checkbox', { name: '查看 启用' })).not.toBeChecked();
});
