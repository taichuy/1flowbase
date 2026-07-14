import { apiFetch } from './transport';

export interface ConsolePermission {
  code: string;
  resource: string;
  action: string;
  scope: string;
  name: string;
  settings_feature?: {
    feature_id: string;
    label_key: string;
    order: number;
  };
}

export function listConsolePermissions(baseUrl?: string): Promise<ConsolePermission[]> {
  return apiFetch<ConsolePermission[]>({
    path: '/api/console/settings/roles/permission-options',
    baseUrl
  });
}
