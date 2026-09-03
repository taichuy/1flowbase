import { apiFetch } from '../transport';

export type WebMcpOperation = 'list' | 'get' | 'result' | 'call';

export interface WebMcpToolRegistration {
  operation: WebMcpOperation;
  name: string;
  title: string;
  description: string;
  input_schema: Record<string, unknown>;
  annotations: {
    read_only_hint: boolean;
    untrusted_content_hint: boolean;
  };
}

export interface WebMcpInstanceRegistration {
  instance_id: string;
  tools: WebMcpToolRegistration[];
}

interface WebMcpInvocationResponse {
  content: unknown;
  is_error: boolean;
}

export function fetchWebMcpRegistrations(
  signal?: AbortSignal,
  baseUrl?: string
) {
  return apiFetch<WebMcpInstanceRegistration[]>({
    path: '/api/webmcp/registrations',
    signal,
    baseUrl
  });
}

export async function invokeWebMcpTool(
  instanceId: string,
  operation: WebMcpOperation,
  argumentsValue: Record<string, unknown>,
  csrfToken: string,
  signal?: AbortSignal,
  baseUrl?: string
) {
  const response = await apiFetch<WebMcpInvocationResponse>({
    path: `/api/webmcp/${encodeURIComponent(instanceId)}/tools/${operation}`,
    method: 'POST',
    body: { arguments: argumentsValue },
    csrfToken,
    signal,
    baseUrl
  });
  if (response.is_error) {
    throw new Error(JSON.stringify(response.content));
  }
  return response.content;
}
