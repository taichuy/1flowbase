import type { ReactNode } from 'react';

import type {
  SchemaBlock,
  SchemaDynamicFormBlock,
  SchemaFieldBlock,
  SchemaViewBlock
} from '../contracts/canvas-node-schema';
import { evaluateSchemaRule } from './rule-evaluator';
import type { RendererRegistry, SchemaAdapter } from '../registry/create-renderer-registry';

export interface SchemaRendererProps {
  adapter: SchemaAdapter;
  blocks: SchemaBlock[];
  registry: RendererRegistry;
  values?: Record<string, unknown>;
  capabilities?: readonly string[];
}

export interface SchemaBlockRendererProps {
  adapter: SchemaAdapter;
  block: SchemaBlock;
  registry: RendererRegistry;
  values?: Record<string, unknown>;
  capabilities?: readonly string[];
}

function shouldRenderBlock(
  block: SchemaBlock,
  values: Record<string, unknown>,
  capabilities: readonly string[]
) {
  return evaluateSchemaRule(block.visibleWhen, { values, capabilities });
}

function renderField(block: SchemaFieldBlock, adapter: SchemaAdapter, registry: RendererRegistry) {
  const Renderer = registry.fields[block.renderer];

  if (!Renderer) {
    return <div data-schema-missing-renderer={block.renderer} />;
  }

  return <Renderer adapter={adapter} block={block} />;
}

function renderView(block: SchemaViewBlock, adapter: SchemaAdapter, registry: RendererRegistry) {
  const Renderer = registry.views[block.renderer];

  if (!Renderer) {
    return <div data-schema-missing-renderer={block.renderer} />;
  }

  return <Renderer adapter={adapter} block={block} />;
}

function renderDynamicForm(
  block: SchemaDynamicFormBlock,
  adapter: SchemaAdapter,
  registry: RendererRegistry
) {
  const Renderer = registry.dynamicForms[block.form_key];

  if (!Renderer) {
    return <div data-schema-missing-dynamic-form={block.form_key} />;
  }

  return <Renderer adapter={adapter} block={block} />;
}

function renderBlock(
  block: SchemaBlock,
  adapter: SchemaAdapter,
  registry: RendererRegistry,
  values: Record<string, unknown>,
  capabilities: readonly string[]
): ReactNode {
  if (!shouldRenderBlock(block, values, capabilities)) {
    return null;
  }

  switch (block.kind) {
    case 'field':
      return renderField(block, adapter, registry);
    case 'view':
      return renderView(block, adapter, registry);
    case 'dynamic_form':
      return renderDynamicForm(block, adapter, registry);
    case 'section':
      return (
        <section data-schema-block="section">
          <h3>{block.title}</h3>
          {block.blocks.map((childBlock, index) => (
            <SchemaRenderer
              key={`${block.title}-${index}`}
              adapter={adapter}
              blocks={[childBlock]}
              registry={registry}
              values={values}
              capabilities={capabilities}
            />
          ))}
        </section>
      );
    case 'stack':
    case 'inline':
    case 'tabs':
      return (
        <div data-schema-block={block.kind}>
          {block.blocks.map((childBlock, index) => (
            <SchemaRenderer
              key={`${block.kind}-${index}`}
              adapter={adapter}
              blocks={[childBlock]}
              registry={registry}
              values={values}
              capabilities={capabilities}
            />
          ))}
        </div>
      );
    default:
      return null;
  }
}

export function SchemaRenderer({
  adapter,
  blocks,
  registry,
  values,
  capabilities = []
}: SchemaRendererProps) {
  const resolvedValues =
    values ?? ((adapter.getDerived('rootValues') as Record<string, unknown> | null) ?? {});

  return (
    <>
      {blocks.map((block, index) => (
        <SchemaBlockRenderer
          key={index}
          adapter={adapter}
          block={block}
          registry={registry}
          values={resolvedValues}
          capabilities={capabilities}
        />
      ))}
    </>
  );
}

export function SchemaBlockRenderer({
  adapter,
  block,
  registry,
  values = {},
  capabilities = []
}: SchemaBlockRendererProps) {
  return <>{renderBlock(block, adapter, registry, values, capabilities)}</>;
}
