import { randomUUID } from 'node:crypto';
import type { IncomingMessage } from 'node:http';

/** Bounded discovery diagnostics; never include query strings or credentials. */
export function oauthProxyDiagnostics(
  write: (line: string) => void = (line) => console.info(line)
) {
  const active = new WeakMap<
    IncomingMessage,
    { id: string; started: number }
  >();
  const safe = (value: unknown) =>
    typeof value === 'string'
      ? value.replace(/[\r\n\t]/g, ' ').slice(0, 256)
      : null;
  function emit(
    request: IncomingMessage,
    event: string,
    status?: number,
    code?: string
  ) {
    const path = request.url?.split('?')[0] ?? '';
    if (
      !path.startsWith('/api/mcp/') &&
      !path.startsWith('/api/public/mcp-oauth/') &&
      !path.startsWith('/.well-known/')
    )
      return;
    let context = active.get(request);
    if (!context) {
      context = { id: randomUUID(), started: Date.now() };
      active.set(request, context);
    }
    write(
      '[mcp-oauth-proxy] ' +
        JSON.stringify({
          timestamp: new Date().toISOString(),
          event,
          request_id: context.id,
          method: safe(request.method),
          path: safe(path),
          host: safe(request.headers.host),
          forwarded_host: safe(request.headers['x-forwarded-host']),
          forwarded_proto: safe(request.headers['x-forwarded-proto']),
          user_agent: safe(request.headers['user-agent']),
          has_authorization: Boolean(request.headers.authorization),
          ...(status === undefined ? {} : { status }),
          ...(code === undefined ? {} : { error_code: safe(code) }),
          ...(event === 'request'
            ? {}
            : { elapsed_ms: Date.now() - context.started })
        })
    );
  }
  return {
    request: (request: IncomingMessage) => emit(request, 'request'),
    response: (request: IncomingMessage, status: number) =>
      emit(request, 'response', status),
    error: (request: IncomingMessage, code?: string) =>
      emit(request, 'error', undefined, code)
  };
}
