export const ISOLATED_FRONTEND_BLOCK_RUNTIME = 'isolated_iframe' as const;

export const ISOLATED_FRONTEND_BLOCK_CAPABILITIES = [
  'block.output.publish'
] as const;

export type IsolatedFrontendBlockCapability =
  (typeof ISOLATED_FRONTEND_BLOCK_CAPABILITIES)[number];

export interface IsolatedFrontendBlockProgram {
  source: string;
  props: Record<string, unknown>;
}

export interface IsolatedFrontendBlockCapabilityRequest {
  type: 'capability_request';
  requestId: string;
  capability: string;
  payload: unknown;
}

export interface IsolatedFrontendBlockCapabilityResponse {
  type: 'capability_response';
  requestId: string;
  ok: boolean;
  value?: unknown;
  error?: 'capability_denied' | 'capability_failed';
}

export type IsolatedFrontendBlockHostCommand =
  | {
      type: 'mount';
      props: Record<string, unknown>;
    }
  | {
      type: 'update';
      props: Record<string, unknown>;
    }
  | {
      type: 'terminate';
    }
  | IsolatedFrontendBlockCapabilityResponse;

export type IsolatedFrontendBlockRealmEvent =
  | { type: 'ready' }
  | { type: 'mounted' }
  | { type: 'updated' }
  | { type: 'failed'; message: string }
  | IsolatedFrontendBlockCapabilityRequest;

export function isIsolatedFrontendBlockCapability(
  value: unknown
): value is IsolatedFrontendBlockCapability {
  return ISOLATED_FRONTEND_BLOCK_CAPABILITIES.some(
    (capability) => capability === value
  );
}
