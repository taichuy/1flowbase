let originalFetch: typeof globalThis.fetch | null = null;

export function getStyleBoundaryOriginalFetch() {
  originalFetch ??= globalThis.fetch.bind(globalThis);
  return originalFetch;
}
