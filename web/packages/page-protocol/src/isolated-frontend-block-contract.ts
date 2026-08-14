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

export interface IsolatedFrontendBlockOutputPublishRequest {
  output: string;
  value: unknown;
}

export interface IsolatedFrontendBlockCapabilityAck {
  accepted: true;
}

export interface IsolatedFrontendBlockCapabilityResponse {
  type: 'capability_response';
  requestId: string;
  ok: boolean;
  value?: IsolatedFrontendBlockCapabilityAck;
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

export function isIsolatedFrontendBlockOutputPublishRequest(
  value: unknown
): value is IsolatedFrontendBlockOutputPublishRequest {
  return (
    isRecord(value) &&
    typeof value.output === 'string' &&
    /^[A-Za-z_][A-Za-z0-9_.-]{0,127}$/u.test(value.output) &&
    Object.prototype.hasOwnProperty.call(value, 'value')
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
