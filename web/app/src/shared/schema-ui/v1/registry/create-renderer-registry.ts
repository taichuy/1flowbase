import type { ComponentType, ReactNode } from 'react';

import type {
  SchemaBlock,
  SchemaDynamicFormBlock,
  SchemaFieldBlock,
  SchemaViewBlock
} from '../contracts/canvas-node-schema';

export interface SchemaAdapter {
  getValue(path: string): unknown;
  setValue(path: string, value: unknown): void;
  getDerived(path: string): unknown;
  dispatch(actionKey: string, payload?: unknown): void;
}

export interface SchemaFieldRendererProps {
  adapter: SchemaAdapter;
  block: SchemaFieldBlock;
}

export interface SchemaViewRendererProps {
  adapter: SchemaAdapter;
  block: SchemaViewBlock;
}

export interface SchemaDynamicFormRendererProps {
  adapter: SchemaAdapter;
  block: SchemaDynamicFormBlock;
}

export interface SchemaShellRendererProps {
  children?: ReactNode;
  block: Extract<SchemaBlock, { kind: 'section' | 'stack' | 'inline' | 'tabs' }>;
}

export type SchemaFieldRenderer = ComponentType<SchemaFieldRendererProps>;
export type SchemaViewRenderer = ComponentType<SchemaViewRendererProps>;
export type SchemaDynamicFormRenderer = ComponentType<SchemaDynamicFormRendererProps>;
export type SchemaShellRenderer = ComponentType<SchemaShellRendererProps>;

export interface RendererRegistryInput {
  fields: Record<string, SchemaFieldRenderer>;
  views: Record<string, SchemaViewRenderer>;
  dynamicForms: Record<string, SchemaDynamicFormRenderer>;
  shells: Record<string, SchemaShellRenderer>;
}

export interface RendererRegistry {
  fields: Record<string, SchemaFieldRenderer>;
  views: Record<string, SchemaViewRenderer>;
  dynamicForms: Record<string, SchemaDynamicFormRenderer>;
  shells: Record<string, SchemaShellRenderer>;
}

export function createRendererRegistry(input: RendererRegistryInput): RendererRegistry {
  return {
    fields: { ...input.fields },
    views: { ...input.views },
    dynamicForms: { ...input.dynamicForms },
    shells: { ...input.shells }
  };
}
