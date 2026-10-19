import { apiFetch, apiFetchVoid } from '../../transport';

export type ConsoleUserMeta = Record<string, unknown>;

export interface ConsoleMe {
  id: string;
  account: string;
  email: string;
  phone: string | null;
  nickname: string;
  name: string;
  avatar_url: string | null;
  introduction: string;
  preferred_locale?: string | null;
  meta?: ConsoleUserMeta;
  effective_display_role: string;
  permissions: string[];
}

export interface UpdateConsoleMeInput {
  name: string;
  nickname: string;
  email: string;
  phone: string | null;
  avatar_url: string | null;
  introduction: string;
  preferred_locale?: string | null;
}

export interface ChangeConsolePasswordInput {
  old_password: string;
  new_password: string;
}

export function fetchConsoleMe(baseUrl?: string): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me',
    baseUrl
  });
}

export function updateConsoleMe(
  input: UpdateConsoleMeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me',
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function patchConsoleMeMeta(
  meta: ConsoleUserMeta,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me/meta',
    method: 'PATCH',
    body: { meta },
    csrfToken,
    baseUrl
  });
}

export function changeConsolePassword(
  input: ChangeConsolePasswordInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: '/api/console/me/actions/change-password',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}
