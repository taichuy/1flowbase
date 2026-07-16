import type { BlockUiSchema } from '@1flowbase/page-protocol';

import type { AntdFacadeInput, AntdFacadeOptions } from './index';

export const Fragment = Symbol.for('flowbase.antd-facade.fragment');

export type JsxFacadeComponent = (input?: AntdFacadeInput) => BlockUiSchema;

export type JsxChild =
  | BlockUiSchema
  | string
  | number
  | boolean
  | null
  | undefined
  | JsxChild[];

const FACADE_OPTION_KEYS = new Set(['key', 'style', 'permissions']);

export function h(
  type: JsxFacadeComponent | typeof Fragment,
  props: Record<string, unknown> | null,
  ...children: JsxChild[]
): BlockUiSchema | JsxChild[] {
  const flatChildren = flattenChildren(children);

  if (type === Fragment) {
    return flatChildren;
  }

  if (typeof type !== 'function') {
    throw new TypeError(
      'JSX element type must be an antd-facade component (capitalized) or a Fragment.'
    );
  }

  const options: AntdFacadeOptions = {};
  const nodeProps: Record<string, unknown> = {};

  if (props !== null && typeof props === 'object') {
    for (const [key, value] of Object.entries(props)) {
      if (FACADE_OPTION_KEYS.has(key)) {
        (options as Record<string, unknown>)[key] = value;
      } else if (key !== 'children') {
        nodeProps[key] = value;
      }
    }
    if (Object.hasOwn(props, 'children') && flatChildren.length === 0) {
      options.children = props.children;
    }
  }

  if (Object.keys(nodeProps).length > 0) {
    options.props = nodeProps;
  }

  if (flatChildren.length > 0) {
    options.children = normalizeJsxChildren(flatChildren);
  }

  return type(options);
}

function flattenChildren(children: JsxChild[]): JsxChild[] {
  const flat: JsxChild[] = [];
  for (const child of children) {
    if (Array.isArray(child)) {
      flat.push(...flattenChildren(child));
    } else if (child !== null && child !== undefined && child !== false && child !== true) {
      flat.push(child);
    }
  }
  return flat;
}

function normalizeJsxChildren(children: JsxChild[]): unknown {
  if (children.length === 1 && (typeof children[0] === 'string' || typeof children[0] === 'number')) {
    return children[0];
  }
  return children;
}
