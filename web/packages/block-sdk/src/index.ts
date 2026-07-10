import type {
  BlockContext,
  BlockContextRecord,
  BlockUiSchema
} from '@1flowbase/page-protocol';

export type BlockLifecycle = (
  ctx: BlockContext
) => void | Promise<void>;

export type BlockRender = (ctx: BlockContext) => BlockUiSchema;

export type BlockInitialState = Readonly<BlockContextRecord>;

export interface BlockDataCapability {
  query<T = unknown>(queryId: string, input?: unknown): Promise<T>;
}

export interface BlockActionCapability {
  invoke<T = unknown>(actionId: string, input?: unknown): Promise<T>;
}

export interface BlockDefinitionInput {
  readonly id?: string;
  readonly title?: string;
  readonly initialState?: BlockContextRecord;
  readonly setup?: BlockLifecycle;
  readonly render: BlockRender;
  readonly dispose?: BlockLifecycle;
}

export interface BlockDefinition {
  readonly id?: string;
  readonly title?: string;
  readonly initialState?: BlockInitialState;
  readonly setup?: BlockLifecycle;
  readonly render: BlockRender;
  readonly dispose?: BlockLifecycle;
}

export type BlockDefinitionErrorCode = 'block_definition_invalid';

export class BlockDefinitionError extends TypeError {
  readonly code: BlockDefinitionErrorCode = 'block_definition_invalid';
  readonly path: string;

  constructor(message: string, path = 'definition') {
    super(message);
    this.name = 'BlockDefinitionError';
    this.path = path;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

type MutableBlockDefinition = {
  id?: string;
  title?: string;
  initialState?: BlockInitialState;
  setup?: BlockLifecycle;
  render: BlockRender;
  dispose?: BlockLifecycle;
};

type DataPropertyResult =
  | { exists: true; value: unknown }
  | { exists: false };

const definitionKeys = new Set<PropertyKey>([
  'id',
  'title',
  'initialState',
  'setup',
  'render',
  'dispose'
]);

const blockDefinitionRegistry = new WeakSet<object>();

export function defineBlock(definition: BlockDefinitionInput): BlockDefinition;
export function defineBlock(definition: unknown): BlockDefinition;
export function defineBlock(definition: unknown): BlockDefinition {
  const source = assertDefinitionRecord(definition);
  assertKnownDefinitionKeys(source);

  const id = readOptionalString(source, 'id');
  const title = readOptionalString(source, 'title');
  const initialState = readOptionalInitialState(source);
  const setup = readOptionalLifecycle(source, 'setup');
  const render = readRequiredRender(source);
  const dispose = readOptionalLifecycle(source, 'dispose');

  const normalized = {} as MutableBlockDefinition;

  if (id !== undefined) {
    normalized.id = id;
  }
  if (title !== undefined) {
    normalized.title = title;
  }
  if (initialState !== undefined) {
    normalized.initialState = initialState;
  }
  if (setup !== undefined) {
    normalized.setup = setup;
  }
  normalized.render = render;
  if (dispose !== undefined) {
    normalized.dispose = dispose;
  }

  const block = Object.freeze(normalized);
  blockDefinitionRegistry.add(block);

  return block;
}

export function isBlockDefinition(value: unknown): value is BlockDefinition {
  return isRecord(value) && blockDefinitionRegistry.has(value);
}

function assertDefinitionRecord(value: unknown): Record<string, unknown> {
  if (!isRecord(value) || Array.isArray(value)) {
    throw new BlockDefinitionError('Block definition must be an object.');
  }

  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new BlockDefinitionError('Block definition must be a plain object.');
  }

  return value;
}

function assertKnownDefinitionKeys(source: Record<string, unknown>): void {
  for (const key of Reflect.ownKeys(source)) {
    if (!definitionKeys.has(key)) {
      throw new BlockDefinitionError(
        'Block definition contains an unsupported key.',
        `definition.${String(key)}`
      );
    }
  }
}

function readOptionalString(
  source: Record<string, unknown>,
  key: 'id' | 'title'
): string | undefined {
  const property = readDataProperty(source, key);
  if (!property.exists || property.value === undefined) {
    return undefined;
  }

  if (typeof property.value !== 'string') {
    throw new BlockDefinitionError(
      `Block definition ${key} must be a string.`,
      `definition.${key}`
    );
  }

  const normalized = property.value.trim();
  if (normalized.length === 0) {
    throw new BlockDefinitionError(
      `Block definition ${key} must not be empty.`,
      `definition.${key}`
    );
  }

  return normalized;
}

function readOptionalInitialState(
  source: Record<string, unknown>
): BlockInitialState | undefined {
  const property = readDataProperty(source, 'initialState');
  if (!property.exists || property.value === undefined) {
    return undefined;
  }

  return normalizeInitialState(property.value);
}

function readOptionalLifecycle(
  source: Record<string, unknown>,
  key: 'setup' | 'dispose'
): BlockLifecycle | undefined {
  const property = readDataProperty(source, key);
  if (!property.exists || property.value === undefined) {
    return undefined;
  }

  if (typeof property.value !== 'function') {
    throw new BlockDefinitionError(
      `Block definition ${key} must be a function.`,
      `definition.${key}`
    );
  }

  return property.value as BlockLifecycle;
}

function readRequiredRender(source: Record<string, unknown>): BlockRender {
  const property = readDataProperty(source, 'render');
  if (!property.exists) {
    throw new BlockDefinitionError(
      'Block definition render must be a function.',
      'definition.render'
    );
  }

  if (typeof property.value !== 'function') {
    throw new BlockDefinitionError(
      'Block definition render must be a function.',
      'definition.render'
    );
  }

  return property.value as BlockRender;
}

function readDataProperty(
  source: Record<string, unknown>,
  key: string
): DataPropertyResult {
  const descriptor = Object.getOwnPropertyDescriptor(source, key);

  if (!descriptor) {
    return { exists: false };
  }

  if (!('value' in descriptor)) {
    throw new BlockDefinitionError(
      'Block definition must not use accessor properties.',
      `definition.${key}`
    );
  }

  return {
    exists: true,
    value: descriptor.value
  };
}

function normalizeInitialState(value: unknown): BlockInitialState {
  if (!isRecord(value) || Array.isArray(value)) {
    throw new BlockDefinitionError(
      'Block definition initialState must be an object.',
      'definition.initialState'
    );
  }

  return freezePlainRecord(value, 'definition.initialState', new WeakSet());
}

function freezePlainData(
  value: unknown,
  path: string,
  seen: WeakSet<object>
): unknown {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean'
  ) {
    return value;
  }

  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new BlockDefinitionError(
        'Block definition initialState number values must be finite.',
        path
      );
    }
    return value;
  }

  if (Array.isArray(value)) {
    return freezePlainArray(value, path, seen);
  }

  if (isRecord(value)) {
    return freezePlainRecord(value, path, seen);
  }

  throw new BlockDefinitionError(
    'Block definition initialState must contain only plain data.',
    path
  );
}

function freezePlainRecord(
  value: Record<string, unknown>,
  path: string,
  seen: WeakSet<object>
): BlockInitialState {
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new BlockDefinitionError(
      'Block definition initialState must contain only plain objects.',
      path
    );
  }

  if (seen.has(value)) {
    throw new BlockDefinitionError(
      'Block definition initialState must not contain cycles.',
      path
    );
  }
  seen.add(value);

  const output: Record<string, unknown> = {};

  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string') {
      throw new BlockDefinitionError(
        'Block definition initialState must not contain symbol keys.',
        path
      );
    }

    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !('value' in descriptor)) {
      throw new BlockDefinitionError(
        'Block definition initialState must not use accessor properties.',
        `${path}.${key}`
      );
    }

    output[key] = freezePlainData(descriptor.value, `${path}.${key}`, seen);
  }

  return Object.freeze(output);
}

function freezePlainArray(
  value: readonly unknown[],
  path: string,
  seen: WeakSet<object>
): readonly unknown[] {
  if (seen.has(value)) {
    throw new BlockDefinitionError(
      'Block definition initialState must not contain cycles.',
      path
    );
  }
  seen.add(value);

  const output: unknown[] = [];
  const allowedKeys = new Set<string>(['length']);

  for (let index = 0; index < value.length; index += 1) {
    const key = String(index);
    allowedKeys.add(key);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !('value' in descriptor)) {
      throw new BlockDefinitionError(
        'Block definition initialState arrays must be dense plain data.',
        `${path}.${key}`
      );
    }
    output.push(freezePlainData(descriptor.value, `${path}.${key}`, seen));
  }

  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string' || !allowedKeys.has(key)) {
      throw new BlockDefinitionError(
        'Block definition initialState arrays must not contain custom keys.',
        path
      );
    }
  }

  return Object.freeze(output);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
