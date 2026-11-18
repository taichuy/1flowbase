import type { RecordPermissionEditorData } from '@1flowbase/api-client';

export const role = {
  role_id: '7cb1111b-e88f-460c-8ad5-3e16127129b6',
  code: 'operator',
  name: 'Operator'
};
export function policy(
  overrides: Partial<RecordPermissionEditorData> = {}
): RecordPermissionEditorData {
  return {
    mode: 'inherit',
    grants: [],
    revision: 0,
    roles: [role],
    resource: {
      resource_code: 'hosts',
      label: 'SSH hosts',
      description: 'Saved SSH hosts',
      actions: ['view', 'update', 'open', 'delete'].map((action_code) => ({
        action_code,
        label: action_code,
        description: ''
      }))
    },
    ...overrides
  };
}
