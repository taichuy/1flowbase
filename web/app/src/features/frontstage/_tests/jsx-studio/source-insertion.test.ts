import { describe, expect, test } from 'vitest';
import * as ts from 'typescript';

import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion
} from '../../lib/jsx-studio/source-insertion';

const sdkSource = '@1flowbase/block-sdk';
const facadeSource = '@1flowbase/block-renderer/antd-facade';

function insertBeforeReturn(source: string) {
  const offset = source.indexOf('  return');
  return { start: offset, end: offset };
}

function sourceDiagnostics(source: string): string[] {
  const entryFile = '/entry.tsx';
  const declarationFile = '/capabilities.d.ts';
  const files = new Map([
    [entryFile, source],
    [
      declarationFile,
      `declare module '${sdkSource}' {
  export interface BlockContext {
    readonly currentUser: unknown;
    readonly api: { get(path: string): unknown };
  }
  export interface BlockModule { readonly main: unknown; }
}
declare module '${facadeSource}' {
  export const Stack: (props?: { children?: unknown }) => unknown;
  export interface ButtonProps {
    readonly actionId?: string;
    readonly children?: unknown;
  }
  export const Button: (props?: ButtonProps) => unknown;
}`
    ]
  ]);
  const options: ts.CompilerOptions = {
    jsx: ts.JsxEmit.Preserve,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Node10,
    noEmit: true,
    noLib: true,
    strict: true,
    target: ts.ScriptTarget.ES2022
  };
  const defaultHost = ts.createCompilerHost(options);
  const host: ts.CompilerHost = {
    ...defaultHost,
    fileExists: (fileName) => files.has(fileName),
    getSourceFile: (fileName, languageVersion) => {
      const content = files.get(fileName);
      return content === undefined
        ? undefined
        : ts.createSourceFile(fileName, content, languageVersion, true);
    },
    readFile: (fileName) => files.get(fileName)
  };
  const program = ts.createProgram([entryFile, declarationFile], options, host);
  return ts
    .getPreEmitDiagnostics(program)
    .filter((diagnostic) => diagnostic.file?.fileName === entryFile)
    .map((diagnostic) =>
      ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')
    );
}

describe('Frontstage JSX source insertion', () => {
  test('AC-001 inserts context references through the actual main parameter binding', () => {
    const source = `import type { BlockContext } from '${sdkSource}';

async function main(_ctx: BlockContext) {
  return { view: null, outputs: {} };
}`;

    const plan = planFrontstageJsxInsertion({
      source,
      selection: insertBeforeReturn(source),
      insertion: { kind: 'context-reference', memberPath: 'currentUser' }
    });

    expect(applyFrontstageJsxInsertionPlan(source, plan)).toContain(
      '_ctx.currentUser'
    );
  });

  test('AC-002 adds a component value import and plans snippet/import as one edit batch', () => {
    const source = `import type { BlockContext } from '${sdkSource}';

async function main(ctx: BlockContext) {
  return { view: null, outputs: {} };
}`;
    const plan = planFrontstageJsxInsertion({
      source,
      selection: insertBeforeReturn(source),
      insertion: {
        kind: 'component',
        name: 'Button',
        moduleSource: facadeSource,
        source: '<Button type="primary" actionId="save">保存</Button>'
      }
    });
    const nextSource = applyFrontstageJsxInsertionPlan(source, plan);

    expect(plan.edits).toHaveLength(2);
    expect(nextSource).toContain(`import { Button } from '${facadeSource}';`);
    expect(nextSource).toContain(
      '<Button type="primary" actionId="save">保存</Button>'
    );
  });

  test('AC-002 and AC-004 merge and deduplicate an existing multiline component import', () => {
    const source = `import {
  Stack
} from '${facadeSource}';

async function main(ctx: unknown) {
  return { view: null, outputs: {} };
}`;
    const firstPlan = planFrontstageJsxInsertion({
      source,
      selection: insertBeforeReturn(source),
      insertion: {
        kind: 'component',
        name: 'Button',
        moduleSource: facadeSource,
        source: '<Button></Button>'
      }
    });
    const firstSource = applyFrontstageJsxInsertionPlan(source, firstPlan);
    const secondPlan = planFrontstageJsxInsertion({
      source: firstSource,
      selection: insertBeforeReturn(firstSource),
      insertion: {
        kind: 'component',
        name: 'Button',
        moduleSource: facadeSource,
        source: '<Button></Button>'
      }
    });
    const secondSource = applyFrontstageJsxInsertionPlan(
      firstSource,
      secondPlan
    );

    expect(secondSource.match(new RegExp(facadeSource, 'g'))).toHaveLength(1);
    expect(secondSource.match(/\bButton\b/g)).toHaveLength(5);
    expect(secondPlan.edits).toHaveLength(1);
  });

  test('AC-003 merges the interface BlockContext type dependency into the SDK import', () => {
    const source = `import type {
  BlockModule,
  BlockResult
} from '${sdkSource}';

export default {} satisfies BlockModule;`;
    const insertionOffset = source.indexOf('export default');
    const plan = planFrontstageJsxInsertion({
      source,
      selection: { start: insertionOffset, end: insertionOffset },
      insertion: {
        kind: 'source',
        source:
          "const loadOrders = (ctx: BlockContext) => ctx.api.get('/orders');",
        requiredImports: [
          {
            kind: 'type',
            name: 'BlockContext',
            moduleSource: sdkSource
          }
        ]
      }
    });
    const nextSource = applyFrontstageJsxInsertionPlan(source, plan);

    expect(nextSource.match(new RegExp(sdkSource, 'g'))).toHaveLength(1);
    expect(nextSource).toContain('  BlockContext,');
    expect(nextSource).toContain('const loadOrders = (ctx: BlockContext)');
  });

  test('AC-006 leaves valid variable, component, and interface insertions without TypeScript diagnostics', () => {
    const baseSource = `import type { BlockContext, BlockModule } from '${sdkSource}';
import { Stack } from '${facadeSource}';

function main(_ctx: BlockContext) {
  return { view: null, outputs: {} };
}

export default { main } satisfies BlockModule;`;
    const viewOffset = baseSource.indexOf('null');
    const variableSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: viewOffset, end: viewOffset + 4 },
        insertion: { kind: 'context-reference', memberPath: 'currentUser' }
      })
    );
    const componentSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: viewOffset, end: viewOffset + 4 },
        insertion: {
          kind: 'component',
          name: 'Button',
          moduleSource: facadeSource,
          source: '<Button></Button>'
        }
      })
    );
    const mainOffset = baseSource.indexOf('function main');
    const interfaceSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: mainOffset, end: mainOffset },
        insertion: {
          kind: 'source',
          source:
            "const loadOrders = (ctx: BlockContext) => ctx.api.get('/orders');\n\n",
          requiredImports: [
            {
              kind: 'type',
              name: 'BlockContext',
              moduleSource: sdkSource
            }
          ]
        }
      })
    );

    expect(sourceDiagnostics(variableSource)).toEqual([]);
    expect(sourceDiagnostics(componentSource)).toEqual([]);
    expect(sourceDiagnostics(interfaceSource)).toEqual([]);
  });

  test('AC-006 rejects unsupported React props that are outside the facade contract', () => {
    const source = `import { Button } from '${facadeSource}';

const view = <Button onClick={() => undefined}>保存</Button>;`;

    expect(sourceDiagnostics(source)).toContain(
      "Type '{ onClick: () => undefined; }' is not assignable to type 'ButtonProps'.\n  Property 'onClick' does not exist on type 'ButtonProps'."
    );
  });
});
