import type { ProxyOptions } from 'vite';

import { oauthProxyDiagnostics } from './oauth-proxy-diagnostics';

/** Project the public authority supplied by a reverse proxy into API requests. */
export function oauthAwareApiProxy(target: string): ProxyOptions {
  return {
    target,
    changeOrigin: false,
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
      proxy.on('proxyReq', (outgoing, incoming) => {
        diagnostics.request(incoming);
        const forwardedHost = incoming.headers['x-forwarded-host'];
        if (typeof forwardedHost === 'string') {
          // The API validates the complete authority, including ambiguity.
          outgoing.setHeader('host', forwardedHost);
        }
      });
    }
  };
}
