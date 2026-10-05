import type { ProxyOptions } from 'vite';

/** Project the public authority supplied by a reverse proxy into API requests. */
export function oauthAwareApiProxy(target: string): ProxyOptions {
  return {
    target,
    changeOrigin: false,
    // Preserve the ingress protocol instead of appending Vite's HTTP hop.
    xfwd: false,
    configure(proxy) {
      proxy.on('proxyReq', (outgoing, incoming) => {
        const forwardedHost = incoming.headers['x-forwarded-host'];
        if (typeof forwardedHost === 'string') {
          // The API validates the complete authority, including ambiguity.
          outgoing.setHeader('host', forwardedHost);
        }
      });
    }
  };
}
