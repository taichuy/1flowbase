import type {
  BlockDataPermission,
  BlockStyleCategory,
  BlockStyleTokenValue,
  BlockUiPermissionMarkers,
  BlockUiPrimitive,
  BlockUiSchema,
  BlockUiSchemaNode,
  BlockUiStyle
} from '@1flowbase/page-protocol';

export interface AntdFacadeOptions {
  key?: unknown;
  props?: unknown;
  style?: unknown;
  permissions?: unknown;
  children?: unknown;
}

export type AntdFacadeInput =
  | AntdFacadeOptions
  | string
  | number
  | boolean
  | null
  | undefined;

type FacadeFactory = (input?: AntdFacadeInput) => BlockUiSchema;

const PRIMITIVES = [
  'Stack',
  'Inline',
  'Grid',
  'Divider',
  'Text',
  'Title',
  'Caption',
  'Badge',
  'Table',
  'Descriptions',
  'Empty',
  'Alert',
  'Form',
  'FormItem',
  'Input',
  'Textarea',
  'Select',
  'Checkbox',
  'Switch',
  'DatePicker',
  'NumberInput',
  'Button',
  'IconButton',
  'Modal'
] as const satisfies readonly BlockUiPrimitive[];

const DATA_PERMISSIONS = [
  'query',
  'create',
  'update',
  'delete'
] as const satisfies readonly BlockDataPermission[];

const STYLE_KEYS = {
  spacing: [
    'margin',
    'marginX',
    'marginY',
    'marginTop',
    'marginRight',
    'marginBottom',
    'marginLeft',
    'padding',
    'paddingX',
    'paddingY',
    'paddingTop',
    'paddingRight',
    'paddingBottom',
    'paddingLeft',
    'gap',
    'rowGap',
    'columnGap'
  ],
  color: ['text', 'background', 'border', 'accent', 'status'],
  typography: ['fontSize', 'fontWeight', 'lineHeight', 'align', 'truncate'],
  border: ['width', 'style', 'color'],
  radius: ['all', 'top', 'right', 'bottom', 'left'],
  layout: [
    'display',
    'width',
    'minWidth',
    'maxWidth',
    'height',
    'minHeight',
    'maxHeight',
    'overflow',
    'align',
    'justify',
    'direction',
    'wrap',
    'columns',
    'span'
  ]
} as const satisfies Record<BlockStyleCategory, readonly string[]>;

const primitiveSet = new Set<string>(PRIMITIVES);
const dataPermissionSet = new Set<string>(DATA_PERMISSIONS);
const styleCategorySet = new Set<string>(Object.keys(STYLE_KEYS));
const styleKeySets: Record<BlockStyleCategory, ReadonlySet<string>> = {
  spacing: new Set(STYLE_KEYS.spacing),
  color: new Set(STYLE_KEYS.color),
  typography: new Set(STYLE_KEYS.typography),
  border: new Set(STYLE_KEYS.border),
  radius: new Set(STYLE_KEYS.radius),
  layout: new Set(STYLE_KEYS.layout)
};
const unsafeJsonValue = Symbol('unsafeJsonValue');

export const Stack = createFacade('Stack');
export const Inline = createFacade('Inline');
export const Grid = createFacade('Grid');
export const Divider = createFacade('Divider');
export const Text = createFacade('Text');
export const Title = createFacade('Title');
export const Caption = createFacade('Caption');
export const Badge = createFacade('Badge');
export const Table = createFacade('Table');
export const Descriptions = createFacade('Descriptions');
export const Empty = createFacade('Empty');
export const Alert = createFacade('Alert');
export const Form = createFacade('Form');
export const FormItem = createFacade('FormItem');
export const Input = createFacade('Input');
export const Textarea = createFacade('Textarea');
export const Select = createFacade('Select');
export const Checkbox = createFacade('Checkbox');
export const Switch = createFacade('Switch');
export const DatePicker = createFacade('DatePicker');
export const NumberInput = createFacade('NumberInput');
export const Button = createFacade('Button');
export const IconButton = createFacade('IconButton');
export const Modal = createFacade('Modal');

function createFacade(primitive: BlockUiPrimitive): FacadeFactory {
  return (input) => buildNode(primitive, normalizeInput(input));
}

function buildNode(
  primitive: BlockUiPrimitive,
  options: AntdFacadeOptions,
  seen: WeakSet<object> = new WeakSet()
): BlockUiSchema {
  const node: BlockUiSchemaNode = { primitive };

  if (typeof options.key === 'string') {
    node.key = options.key;
  }

  const props = sanitizeProps(options.props, seen);
  const propChildren = sanitizePropChildren(options.children, seen);
  if (propChildren !== undefined) {
    const nextProps = props ?? {};
    nextProps.children = propChildren;
    node.props = nextProps;
  } else if (props !== undefined) {
    node.props = props;
  }

  const style = sanitizeStyle(options.style);
  if (style !== undefined) {
    node.style = style;
  }

  const children = sanitizeChildren(options.children, seen);
  if (children !== undefined) {
    node.children = children;
  }

  const permissions = sanitizePermissions(options.permissions);
  if (permissions !== undefined) {
    node.permissions = permissions;
  }

  return node;
}

function normalizeInput(input: AntdFacadeInput): AntdFacadeOptions {
  if (input === undefined) {
    return {};
  }

  if (isTokenValue(input)) {
    return { props: { children: input } };
  }

  if (!isPlainRecord(input) || isReactLikeElement(input) || isDomLikeNode(input)) {
    return {};
  }

  return input;
}

function sanitizeProps(
  value: unknown,
  seen: WeakSet<object>
): Record<string, unknown> | undefined {
  const sanitized = sanitizeJsonValue(value, seen);
  if (!isPlainRecord(sanitized)) {
    return undefined;
  }

  return hasKeys(sanitized) ? sanitized : undefined;
}

function sanitizePropChildren(
  value: unknown,
  seen: WeakSet<object>
): unknown {
  if (value === undefined || isSchemaChild(value)) {
    return undefined;
  }

  if (Array.isArray(value) && value.some(isSchemaChild)) {
    return undefined;
  }

  return sanitizeJsonValue(value, seen);
}

function sanitizeChildren(
  value: unknown,
  seen: WeakSet<object>
): BlockUiSchemaNode[] | undefined {
  const values = Array.isArray(value) ? value : [value];
  const children: BlockUiSchemaNode[] = [];

  for (const item of values) {
    const child = sanitizeSchemaChild(item, seen);
    if (child !== undefined) {
      children.push(child);
    }
  }

  return children.length > 0 ? children : undefined;
}

function sanitizeSchemaChild(
  value: unknown,
  seen: WeakSet<object>
): BlockUiSchemaNode | undefined {
  if (!isSchemaChild(value) || seen.has(value)) {
    return undefined;
  }

  seen.add(value);
  const primitive = value.primitive as BlockUiPrimitive;
  const child = buildNode(primitive, value, seen);
  seen.delete(value);
  return child;
}

function isSchemaChild(value: unknown): value is BlockUiSchemaNode {
  if (!isPlainRecord(value) || isReactLikeElement(value) || isDomLikeNode(value)) {
    return false;
  }

  return typeof value.primitive === 'string' && primitiveSet.has(value.primitive);
}

function sanitizeJsonValue(
  value: unknown,
  seen: WeakSet<object>
): unknown {
  if (isTokenValue(value)) {
    return value;
  }

  if (typeof value !== 'object' || value === null) {
    return undefined;
  }

  if (isReactLikeElement(value) || isDomLikeNode(value)) {
    return undefined;
  }

  if (seen.has(value)) {
    return unsafeJsonValue;
  }

  seen.add(value);

  if (Array.isArray(value)) {
    const items = value
      .map((item) => sanitizeJsonValue(item, seen))
      .filter((item) => item !== undefined && item !== unsafeJsonValue);
    seen.delete(value);
    return items;
  }

  if (!isPlainRecord(value)) {
    seen.delete(value);
    return undefined;
  }

  const output: Record<string, unknown> = {};
  for (const key of Object.keys(value)) {
    if (isBlockedPropKey(key)) {
      continue;
    }

    const item = sanitizeJsonValue(readRecordValue(value, key), seen);
    if (item === unsafeJsonValue) {
      seen.delete(value);
      return undefined;
    }

    if (item !== undefined) {
      output[key] = item;
    }
  }

  seen.delete(value);
  return hasKeys(output) ? output : undefined;
}

function sanitizeStyle(value: unknown): BlockUiStyle | undefined {
  if (!isPlainRecord(value)) {
    return undefined;
  }

  const output: BlockUiStyle = {};

  for (const category of Object.keys(value)) {
    if (!styleCategorySet.has(category)) {
      continue;
    }

    const categoryValue = readRecordValue(value, category);
    if (!isPlainRecord(categoryValue)) {
      continue;
    }

    const allowedKeys = styleKeySets[category as BlockStyleCategory];
    const tokens: Record<string, BlockStyleTokenValue> = {};

    for (const tokenKey of Object.keys(categoryValue)) {
      const tokenValue = readRecordValue(categoryValue, tokenKey);
      if (allowedKeys.has(tokenKey) && isTokenValue(tokenValue)) {
        tokens[tokenKey] = tokenValue;
      }
    }

    if (hasKeys(tokens)) {
      output[category as BlockStyleCategory] = tokens;
    }
  }

  return hasKeys(output) ? output : undefined;
}

function sanitizePermissions(
  value: unknown
): BlockUiPermissionMarkers | undefined {
  if (!isPlainRecord(value)) {
    return undefined;
  }

  const data = sanitizeDataPermissions(readRecordValue(value, 'data'));
  const actions = sanitizeStringMarkers(readRecordValue(value, 'actions'));
  const events = sanitizeStringMarkers(readRecordValue(value, 'events'));
  const permissions: BlockUiPermissionMarkers = {};

  if (data.length > 0) {
    permissions.data = data;
  }
  if (actions.length > 0) {
    permissions.actions = actions;
  }
  if (events.length > 0) {
    permissions.events = events;
  }

  return hasKeys(permissions) ? permissions : undefined;
}

function sanitizeDataPermissions(value: unknown): BlockDataPermission[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(
    (item): item is BlockDataPermission =>
      typeof item === 'string' && dataPermissionSet.has(item)
  );
}

function sanitizeStringMarkers(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(
    (item): item is string => typeof item === 'string' && item.length > 0
  );
}

function isBlockedPropKey(key: string): boolean {
  return key === 'className' || key === 'style';
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  try {
    return Object.getPrototypeOf(value) === Object.prototype;
  } catch {
    return false;
  }
}

function isTokenValue(value: unknown): value is BlockStyleTokenValue {
  return (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean' ||
    (typeof value === 'number' && Number.isFinite(value))
  );
}

function isReactLikeElement(value: unknown): boolean {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const record = value as Record<string, unknown>;
  if ('$$typeof' in record) {
    return true;
  }

  return 'type' in record && 'props' in record;
}

function isDomLikeNode(value: unknown): boolean {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const record = value as Record<string, unknown>;
  return typeof record.nodeType === 'number' || 'ownerDocument' in record;
}

function readRecordValue(
  value: Record<string, unknown>,
  key: string
): unknown {
  try {
    return value[key];
  } catch {
    return undefined;
  }
}

function hasKeys(value: object): boolean {
  return Object.keys(value).length > 0;
}

export { h, Fragment } from './jsx-runtime';
export type { JsxChild, JsxFacadeComponent } from './jsx-runtime';
