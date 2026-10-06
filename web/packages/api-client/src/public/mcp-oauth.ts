import { apiFetch } from '../transport';

export interface McpOAuthConfiguration {
  enabled: boolean;
  server_url: string | null;
  registration_method: 'dynamic_client_registration';
  scope: string;
}

export interface McpOAuthAuthorization {
  client_name: string;
  instance_id: string;
  scope: string;
}

export interface McpOAuthApproval {
  approval_token: string;
  workspace_name: string;
  instance_name: string;
  scope: string;
}

export interface McpOAuthDecisionInput {
  request_id: string;
  approval_token?: string;
  approved: boolean;
}

const prefix = '/api/public/mcp-oauth';

export function fetchMcpOAuthConfiguration(
  instance_id: string,
  origin?: string
) {
  return apiFetch<McpOAuthConfiguration>({
    path: `${prefix}/config?${new URLSearchParams({ instance_id, ...(origin ? { origin } : {}) })}`,
    unwrapSuccess: false
  });
}

export function fetchMcpOAuthAuthorization(
  request_id: string,
  signal?: AbortSignal,
  origin?: string
) {
  return apiFetch<McpOAuthAuthorization>({
    path: `${prefix}/authorization?${new URLSearchParams({ request_id, ...(origin ? { origin } : {}) })}`,
    unwrapSuccess: false,
    signal
  });
}

export function verifyMcpOAuthApiKey(
  input: {
    request_id: string;
    api_key: string;
  },
  origin?: string
) {
  return apiFetch<McpOAuthApproval>({
    path: `${prefix}/verify${origin ? `?${new URLSearchParams({ origin })}` : ''}`,
    method: 'POST',
    body: input,
    unwrapSuccess: false
  });
}

export function decideMcpOAuthAuthorization(
  input: McpOAuthDecisionInput,
  origin?: string
) {
  return apiFetch<{ redirect_uri: string }>({
    path: `${prefix}/decision${origin ? `?${new URLSearchParams({ origin })}` : ''}`,
    method: 'POST',
    body: input,
    unwrapSuccess: false
  });
}
