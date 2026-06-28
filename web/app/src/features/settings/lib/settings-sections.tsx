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
  key: SettingsSectionKey;
}

export interface SettingsSectionDefinition extends Omit<
  SettingsSectionNavItem,
  'label'
> {
  labelKey: string;
  requiredPermissions: string[];
}

export const settingsSectionDefinitions: SettingsSectionDefinition[] = [
  {
    key: 'docs',
    labelKey: 'auto.api_documentation',
    to: '/settings/docs',
    requiredPermissions: ['api_reference.view.all']
  },
  {
    key: 'api-key-authentication',
    labelKey: 'auto.api_key_authentication',
    to: '/settings/api-key-authentication',
    requiredPermissions: []
  },
  {
    key: 'auth-center',
    labelKey: 'auto.auth_center',
    to: '/settings/auth-center',
    requiredPermissions: ['user.view.all']
  },
  {
    key: 'system-runtime',
    labelKey: 'auto.system_runtime',
    to: '/settings/system-runtime',
    requiredPermissions: ['system_runtime.view.all']
  },
  {
    key: 'host-infrastructure',
    labelKey: 'auto.infrastructure',
    to: '/settings/host-infrastructure',
    requiredPermissions: ['plugin_config.view.all']
  },
  {
    key: 'memory-observation',
    labelKey: 'auto.memory_observation',
    to: '/settings/memory-observation',
    requiredPermissions: ['plugin_config.view.all']
  },
  {
    key: 'files',
    labelKey: 'auto.file_management',
    to: '/settings/files',
    requiredPermissions: [
      'file_table.view.all',
      'file_table.view.own',
      'file_table.create.all'
    ]
  },
  {
    key: 'data-models',
    labelKey: 'auto.data_source',
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
    labelKey: 'auto.model_providers',
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
    labelKey: 'auto.mcp_management',
    to: '/settings/mcp-management',
    requiredPermissions: [
      'mcp_management.view.all',
      'mcp_management.manage.all'
    ]
  },
  {
    key: 'members',
    labelKey: 'auto.user_management',
    to: '/settings/members',
    requiredPermissions: ['user.view.all']
  },
  {
    key: 'roles',
    labelKey: 'auto.permission_management',
    to: '/settings/roles',
    requiredPermissions: ['role_permission.view.all']
  }
];
