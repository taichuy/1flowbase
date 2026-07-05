import { apiFetch } from './transport';

export type ConsoleSurfaceKind = 'system' | 'dynamic_page' | 'host_extension';
export type ConsoleNavigationSlot = 'primary' | 'secondary' | 'settings';
export type ConsolePermissionRequirement = 'authenticated' | 'any_permission';

export interface ConsoleRouteDefinition {
  route_id: string;
  surface_key: string;
  path: string;
  surface_kind: ConsoleSurfaceKind;
}

export interface ConsoleNavigationItem {
  item_id: string;
  route_id: string;
  parent_item_id: string | null;
  label_key: string;
  navigation_slot: ConsoleNavigationSlot;
  order: number;
}

export interface ConsolePermissionBinding {
  binding_id: string;
  route_id: string;
  permission_codes: string[];
  requirement: ConsolePermissionRequirement;
}

export interface ConsoleNavigation {
  route_definitions: ConsoleRouteDefinition[];
  navigation_items: ConsoleNavigationItem[];
  permission_bindings: ConsolePermissionBinding[];
}

export function getConsoleNavigation(baseUrl?: string): Promise<ConsoleNavigation> {
  return apiFetch<ConsoleNavigation>({
    path: '/api/console/navigation',
    baseUrl
  });
}
