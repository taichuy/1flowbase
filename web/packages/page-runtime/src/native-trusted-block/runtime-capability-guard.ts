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
    document: browserScope.document,
    window: browserScope.window ?? browserScope,
    globalThis: browserScope,
    self: browserScope.self ?? browserScope
  };
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
