export const RUNTIME_CAPABILITY_GUARD_BINDING_NAMES = [
  'fetch',
  'XMLHttpRequest',
  'WebSocket',
  'navigator',
  'localStorage',
  'sessionStorage',
  'document',
  'window',
  'globalThis',
  'self'
] as const;

export type NativeTrustedBlockRuntimeCapabilityGuardBindingName =
  (typeof RUNTIME_CAPABILITY_GUARD_BINDING_NAMES)[number];

export type NativeTrustedBlockRuntimeCapabilityGuardBindings = Record<
  NativeTrustedBlockRuntimeCapabilityGuardBindingName,
  unknown
>;

export class NativeTrustedBlockRuntimeCapabilityGuardError extends Error {
  readonly path: string;

  constructor(path: string, capability: string) {
    super(
      `Native trusted block runtime capability '${capability}' is not available.`
    );
    this.name = 'NativeTrustedBlockRuntimeCapabilityGuardError';
    this.path = path;
  }
}

export function createNativeTrustedBlockRuntimeCapabilityGuardBindings():
  NativeTrustedBlockRuntimeCapabilityGuardBindings {
  return {
    fetch: createDeniedCallable('fetch'),
    XMLHttpRequest: createDeniedCallable('XMLHttpRequest'),
    WebSocket: createDeniedCallable('WebSocket'),
    navigator: createDeniedObject('navigator', ['sendBeacon']),
    localStorage: createDeniedObject('localStorage'),
    sessionStorage: createDeniedObject('sessionStorage'),
    document: createDeniedObject('document', ['cookie']),
    window: createDeniedObject('window'),
    globalThis: createDeniedObject('globalThis'),
    self: createDeniedObject('self')
  };
}

export function createNativeTrustedBlockBrowserCapabilityBindings(
  scope: typeof globalThis = globalThis
): NativeTrustedBlockRuntimeCapabilityGuardBindings {
  const browserScope = scope as typeof globalThis & {
    window?: unknown;
    document?: unknown;
    self?: unknown;
    navigator?: unknown;
    localStorage?: unknown;
    sessionStorage?: unknown;
    XMLHttpRequest?: unknown;
    WebSocket?: unknown;
  };
  const browserWindow = browserScope.window ?? browserScope;
  const selectionAwareBindings = createSelectionAwareBrowserBindings(
    browserWindow,
    browserScope.document
  );
  return {
    fetch:
      typeof browserScope.fetch === 'function'
        ? browserScope.fetch.bind(browserScope)
        : undefined,
    XMLHttpRequest: browserScope.XMLHttpRequest,
    WebSocket: browserScope.WebSocket,
    navigator: browserScope.navigator,
    localStorage: readBrowserCapability(() => browserScope.localStorage),
    sessionStorage: readBrowserCapability(() => browserScope.sessionStorage),
    document: selectionAwareBindings.document,
    window: selectionAwareBindings.window,
    globalThis: browserScope,
    self: browserScope.self ?? browserScope
  };
}

function createSelectionAwareBrowserBindings(
  browserWindow: unknown,
  browserDocument: unknown
): { window: unknown; document: unknown } {
  if (!isObjectValue(browserWindow) || !isObjectValue(browserDocument)) {
    return { window: browserWindow, document: browserDocument };
  }
  const resolveSelection = () =>
    resolveNativeTrustedBlockSelection(browserWindow, browserDocument);
  const documentProxy = proxyBrowserObject(browserDocument, {
    getSelection: resolveSelection
  });
  const windowProxy = proxyBrowserObject(browserWindow, {
    document: () => documentProxy,
    getSelection: resolveSelection
  });
  return { window: windowProxy, document: documentProxy };
}

function resolveNativeTrustedBlockSelection(
  browserWindow: object,
  browserDocument: object
): unknown {
  const fallback = callObjectMethod(browserWindow, 'getSelection');
  if (hasRenderableSelectionRange(fallback)) return fallback;
  const roots = callObjectMethod(
    browserDocument,
    'querySelectorAll',
    '[data-flowbase-native-trusted-block-root]'
  );
  if (!isIterable(roots)) return fallback;
  const fallbackText = selectionText(fallback);
  for (const root of roots) {
    if (!isObjectValue(root) || !isObjectValue(root.shadowRoot)) continue;
    const candidate = callObjectMethod(root.shadowRoot, 'getSelection');
    if (
      hasRenderableSelectionRange(candidate) &&
      (!fallbackText || selectionText(candidate) === fallbackText)
    ) {
      return candidate;
    }
  }
  return fallback;
}

function hasRenderableSelectionRange(selection: unknown): boolean {
  if (!isObjectValue(selection) || selection.rangeCount !== 1) return false;
  const range = callObjectMethod(selection, 'getRangeAt', 0);
  if (!isObjectValue(range)) return false;
  const rect = callObjectMethod(range, 'getBoundingClientRect');
  return (
    isObjectValue(rect) &&
    ((typeof rect.width === 'number' && rect.width > 0) ||
      (typeof rect.height === 'number' && rect.height > 0))
  );
}

function selectionText(selection: unknown): string {
  if (!isObjectValue(selection)) return '';
  const value = callObjectMethod(selection, 'toString');
  return typeof value === 'string' ? value : '';
}

function proxyBrowserObject(
  target: object,
  overrides: Record<PropertyKey, () => unknown>
): object {
  type BrowserMethod = (...args: unknown[]) => unknown;
  const boundMethods = new WeakMap<BrowserMethod, BrowserMethod>();
  return new Proxy(target, {
    get(object, property) {
      const override = overrides[property];
      if (override) return property === 'getSelection' ? override : override();
      const value = Reflect.get(object, property, object);
      if (typeof value !== 'function') return value;
      const method = value as BrowserMethod;
      let bound = boundMethods.get(method);
      if (!bound) {
        bound = method.bind(object) as BrowserMethod;
        boundMethods.set(method, bound);
      }
      return bound;
    }
  });
}

function callObjectMethod(
  target: object,
  property: PropertyKey,
  ...args: unknown[]
): unknown {
  const method = Reflect.get(target, property, target);
  return typeof method === 'function' ? method.apply(target, args) : undefined;
}

function isIterable(value: unknown): value is Iterable<unknown> {
  return (
    isObjectValue(value) &&
    typeof Reflect.get(value, Symbol.iterator, value) === 'function'
  );
}

export function getNativeTrustedBlockRuntimeCapabilityGuardValues(
  bindings: NativeTrustedBlockRuntimeCapabilityGuardBindings
): unknown[] {
  return RUNTIME_CAPABILITY_GUARD_BINDING_NAMES.map((name) => bindings[name]);
}

export function isNativeTrustedBlockRuntimeCapabilityGuardError(
  error: unknown
): error is NativeTrustedBlockRuntimeCapabilityGuardError {
  return (
    error instanceof NativeTrustedBlockRuntimeCapabilityGuardError ||
    (isRecord(error) &&
      error.name === 'NativeTrustedBlockRuntimeCapabilityGuardError' &&
      typeof error.path === 'string')
  );
}

function createDeniedCallable(capability: string): unknown {
  const deny = function deniedNativeTrustedBlockCapability(): never {
    throw capabilityError(capability);
  };

  return new Proxy(deny, {
    apply: deny,
    construct: deny,
    defineProperty: deny,
    deleteProperty: deny,
    get: deny,
    getOwnPropertyDescriptor: deny,
    getPrototypeOf: deny,
    has: deny,
    ownKeys: deny,
    preventExtensions: deny,
    set: deny,
    setPrototypeOf: deny
  });
}

function readBrowserCapability(read: () => unknown): unknown {
  try {
    return read();
  } catch {
    return undefined;
  }
}

function createDeniedObject(
  capability: string,
  deniedProperties: readonly string[] = []
): unknown {
  const deniedPropertySet = new Set(deniedProperties);

  return new Proxy(Object.create(null), {
    defineProperty(_target, property): never {
      throw capabilityError(formatCapability(capability, property));
    },
    deleteProperty(_target, property): never {
      throw capabilityError(formatCapability(capability, property));
    },
    get(_target, property): unknown {
      const propertyName = String(property);
      if (deniedPropertySet.has(propertyName)) {
        throw capabilityError(formatCapability(capability, property));
      }
      throw capabilityError(capability);
    },
    getOwnPropertyDescriptor(_target, property): never {
      throw capabilityError(formatCapability(capability, property));
    },
    getPrototypeOf(): never {
      throw capabilityError(capability);
    },
    has(_target, property): never {
      throw capabilityError(formatCapability(capability, property));
    },
    ownKeys(): never {
      throw capabilityError(capability);
    },
    preventExtensions(): never {
      throw capabilityError(capability);
    },
    set(_target, property): never {
      throw capabilityError(formatCapability(capability, property));
    },
    setPrototypeOf(): never {
      throw capabilityError(capability);
    }
  });
}

function capabilityError(
  capability: string
): NativeTrustedBlockRuntimeCapabilityGuardError {
  return new NativeTrustedBlockRuntimeCapabilityGuardError(
    `runtime.capability.${capability}`,
    capability
  );
}

function formatCapability(capability: string, property: string | symbol): string {
  return typeof property === 'symbol'
    ? capability
    : `${capability}.${property}`;
}

function isRecord(value: unknown): value is { name?: unknown; path?: unknown } {
  return typeof value === 'object' && value !== null;
}

function isObjectValue(value: unknown): value is Record<PropertyKey, unknown> {
  return (typeof value === 'object' && value !== null) || typeof value === 'function';
}
