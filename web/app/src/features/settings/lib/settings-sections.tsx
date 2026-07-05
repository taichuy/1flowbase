import type { SectionNavItem } from '../../../shared/ui/section-page-layout/SectionPageLayout';

export type SettingsSectionKey =
  | 'docs'
  | 'api-key-authentication'
  | 'auth-center'
  | 'system-runtime'
  | 'host-infrastructure'
  | 'memory-observation'
  | 'files'
  | 'data-models'
  | 'mcp-management'
  | 'model-providers'
  | 'members'
  | 'roles';

export interface SettingsSectionNavItem extends SectionNavItem {
  key: string;
}

export interface SettingsSectionDefinition extends Omit<
  SettingsSectionNavItem,
  'label' | 'key'
> {
  key: SettingsSectionKey;
  label_key: string;
  requiredPermissions: string[];
}

export interface SettingsSectionRegistryItem {
  key: string;
  label_key: string;
  to: string;
}

interface ConsoleNavigationLike {
  route_definitions: Array<{
    route_id: string;
    surface_key: string;
    path: string;
  }>;
  navigation_items: Array<{
    route_id: string;
    parent_item_id: string | null;
    label_key: string;
    navigation_slot: string;
    order: number;
  }>;
}

const settingsSectionKeys = new Set<SettingsSectionKey>([
  'docs',
  'api-key-authentication',
  'auth-center',
  'system-runtime',
  'host-infrastructure',
  'memory-observation',
  'files',
  'data-models',
  'mcp-management',
  'model-providers',
  'members',
  'roles'
]);

export function isSettingsSectionKey(
  value: string
): value is SettingsSectionKey {
  return settingsSectionKeys.has(value as SettingsSectionKey);
}

function settingsSectionKeyFromPath(path: string, fallbackKey: string): string {
  const settingsPrefix = '/settings/';
  if (!path.startsWith(settingsPrefix)) {
    return fallbackKey;
  }

  return path.slice(settingsPrefix.length).split('/')[0] || fallbackKey;
}

export function settingsSectionItemsFromConsoleNavigation(
  navigation: ConsoleNavigationLike
): SettingsSectionRegistryItem[] {
  const routesById = new Map(
    navigation.route_definitions.map((route) => [route.route_id, route])
  );

  return navigation.navigation_items
    .filter(
      (item) =>
        item.navigation_slot === 'settings' &&
        item.parent_item_id === 'settings'
    )
    .sort((left, right) => left.order - right.order)
    .flatMap((item) => {
      const route = routesById.get(item.route_id);
      if (!route) {
        return [];
      }

      return [
        {
          key: settingsSectionKeyFromPath(route.path, route.surface_key),
          label_key: item.label_key,
          to: route.path
        }
      ];
    });
}

export const settingsSectionDefinitions: SettingsSectionDefinition[] = [
  {
    key: 'docs',
    label_key: 'auto.api_documentation',
    to: '/settings/docs',
    requiredPermissions: ['api_reference.view.all']
  },
  {
    key: 'api-key-authentication',
    label_key: 'auto.api_key_authentication',
    to: '/settings/api-key-authentication',
    requiredPermissions: []
  },
  {
    key: 'auth-center',
    label_key: 'auto.auth_center',
    to: '/settings/auth-center',
    requiredPermissions: ['user.view.all']
  },
  {
    key: 'system-runtime',
    label_key: 'auto.system_runtime',
    to: '/settings/system-runtime',
    requiredPermissions: ['system_runtime.view.all']
  },
  {
    key: 'host-infrastructure',
    label_key: 'auto.infrastructure',
    to: '/settings/host-infrastructure',
    requiredPermissions: ['plugin_config.view.all']
  },
  {
    key: 'memory-observation',
    label_key: 'auto.memory_observation',
    to: '/settings/memory-observation',
    requiredPermissions: ['plugin_config.view.all']
  },
  {
    key: 'files',
    label_key: 'auto.file_management',
    to: '/settings/files',
    requiredPermissions: [
      'file_table.view.all',
      'file_table.view.own',
      'file_table.create.all'
    ]
  },
  {
    key: 'data-models',
    label_key: 'auto.data_source',
    to: '/settings/data-models',
    requiredPermissions: [
      'state_model.view.all',
      'state_model.view.own',
      'state_model.manage.all',
      'state_model.manage.own'
    ]
  },
  {
    key: 'model-providers',
    label_key: 'auto.model_providers',
    to: '/settings/model-providers',
    requiredPermissions: [
      'state_model.view.all',
      'state_model.view.own',
      'state_model.manage.all',
      'state_model.manage.own'
    ]
  },
  {
    key: 'mcp-management',
    label_key: 'auto.mcp_management',
    to: '/settings/mcp-management',
    requiredPermissions: [
      'mcp_management.view.all',
      'mcp_management.manage.all'
    ]
  },
  {
    key: 'members',
    label_key: 'auto.user_management',
    to: '/settings/members',
    requiredPermissions: ['user.view.all']
  },
  {
    key: 'roles',
    label_key: 'auto.permission_management',
    to: '/settings/roles',
    requiredPermissions: ['role_permission.view.all']
  }
];
