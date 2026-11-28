import { apiFetch } from '../../transport/json';

export interface EvaluateConsoleAccessResponse {
  policy_fingerprint: string;
  allowed: Record<string, boolean>;
}

export function evaluateConsoleAccess(
  codes: string[],
  csrfToken: string,
  baseUrl?: string
): Promise<EvaluateConsoleAccessResponse> {
  return apiFetch({
    path: '/api/console/access/evaluate',
    method: 'POST',
    body: { codes },
    csrfToken,
    baseUrl
  });
}
