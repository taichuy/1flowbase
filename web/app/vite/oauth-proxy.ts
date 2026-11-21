import type { ClientRequest, IncomingMessage } from 'node:http';

import type { ProxyOptions } from 'vite';

import { oauthProxyDiagnostics } from './oauth-proxy-diagnostics';

function isHttpsTarget(target: string): boolean {
  try {
    return new URL(target).protocol === 'https:';
  } catch {
    return false;
  }
}

/** Project the public authority supplied by a reverse proxy into API requests. */
export function oauthAwareApiProxy(target: string): ProxyOptions {
  const remoteHttps = isHttpsTarget(target);
  return {
    target,
    // HTTPS upstreams require matching Host/SNI against the remote certificate
    // and vhost; local HTTP upstreams preserve incoming public authority.
    changeOrigin: remoteHttps,
    // Preserve the ingress protocol instead of appending Vite's HTTP hop.
    xfwd: false,
    configure(proxy) {
      const diagnostics = oauthProxyDiagnostics();
      proxy.on('proxyRes', (response, incoming) => {
        diagnostics.response(incoming, response.statusCode ?? 502);
      });
      proxy.on('error', (error, incoming) => {
        diagnostics.error(
          incoming,
          'code' in error ? String(error.code) : undefined
        );
      });
      const projectPublicAuthority = (
        outgoing: ClientRequest,
        incoming: IncomingMessage
      ) => {
        diagnostics.request(incoming);
        if (remoteHttps) return;
        const forwardedHost = incoming.headers['x-forwarded-host'];
        if (typeof forwardedHost === 'string') {
          // The API validates the complete authority, including ambiguity.
          outgoing.setHeader('host', forwardedHost);
        }
      };
      proxy.on('proxyReq', projectPublicAuthority);
      proxy.on('proxyReqWs', projectPublicAuthority);
    }
  };
}
