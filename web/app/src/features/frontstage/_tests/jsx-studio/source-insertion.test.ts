import { describe, expect, test } from 'vitest';
import * as ts from 'typescript';

import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion
} from '../../lib/jsx-studio/source-insertion';

const sdkSource = '@1flowbase/block-sdk';
const componentSource = '@1flowbase/native-components';

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
}
declare module '${componentSource}' {
  export const Stack: (props?: { children?: unknown }) => unknown;
  export interface ButtonProps {
    readonly onClick?: () => void;
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
  test('D4-AC-005 inserts context references through the default React component binding', () => {
    const source = `import type { BlockContext } from '${sdkSource}';

export default function Block({ ctx: _ctx }: { ctx: BlockContext }) {
  return <div>Ready</div>;
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

  test('D2-AC-002 adds a standard React component import and snippet as one edit batch', () => {
    const source = `import type { BlockContext } from '${sdkSource}';

export default function Block({ ctx }: { ctx: BlockContext }) {
  return <div>Ready</div>;
}`;
    const plan = planFrontstageJsxInsertion({
      source,
      selection: insertBeforeReturn(source),
      insertion: {
        kind: 'component',
        name: 'Button',
        moduleSource: componentSource,
        source: '<Button onClick={() => undefined}>保存</Button>'
      }
    });
    const nextSource = applyFrontstageJsxInsertionPlan(source, plan);

    expect(plan.edits).toHaveLength(2);
    expect(nextSource).toContain(`import { Button } from '${componentSource}';`);
    expect(nextSource).toContain(
      '<Button onClick={() => undefined}>保存</Button>'
    );
  });

  test('AC-002 and AC-004 merge and deduplicate an existing multiline component import', () => {
    const source = `import {
  Stack
} from '${componentSource}';

export default function Block({ ctx }: { ctx: unknown }) {
  return <Stack>Ready</Stack>;
}`;
    const firstPlan = planFrontstageJsxInsertion({
      source,
      selection: insertBeforeReturn(source),
      insertion: {
        kind: 'component',
        name: 'Button',
        moduleSource: componentSource,
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
        moduleSource: componentSource,
        source: '<Button></Button>'
      }
    });
    const secondSource = applyFrontstageJsxInsertionPlan(
      firstSource,
      secondPlan
    );

    expect(secondSource.match(new RegExp(componentSource, 'g'))).toHaveLength(1);
    expect(secondSource.match(/\bButton\b/g)).toHaveLength(5);
    expect(secondPlan.edits).toHaveLength(1);
  });

  test('D4-AC-005 merges an interface BlockContext helper into standard component source', () => {
    const source = `export default function Block() {
  return <div>Ready</div>;
}`;
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
    expect(nextSource).toContain('import type { BlockContext }');
    expect(nextSource).toContain('const loadOrders = (ctx: BlockContext)');
  });

  test('AC-006 leaves valid variable, component, and interface insertions without TypeScript diagnostics', () => {
    const baseSource = `import type { BlockContext } from '${sdkSource}';
import { Stack } from '${componentSource}';

export default function Block({ ctx: _ctx }: { ctx: BlockContext }) {
  return <Stack>content</Stack>;
}`;
    const viewOffset = baseSource.indexOf('content');
    const variableSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: viewOffset, end: viewOffset + 7 },
        insertion: { kind: 'context-reference', memberPath: 'currentUser' }
      })
    );
    const componentInsertedSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: viewOffset, end: viewOffset + 7 },
        insertion: {
          kind: 'component',
          name: 'Button',
          moduleSource: componentSource,
          source: '<Button></Button>'
        }
      })
    );
    const componentOffset = baseSource.indexOf('export default');
    const interfaceSource = applyFrontstageJsxInsertionPlan(
      baseSource,
      planFrontstageJsxInsertion({
        source: baseSource,
        selection: { start: componentOffset, end: componentOffset },
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
    expect(sourceDiagnostics(componentInsertedSource)).toEqual([]);
    expect(sourceDiagnostics(interfaceSource)).toEqual([]);
  });

  test('D2-AC-002 accepts standard React props from the registered declaration', () => {
    const source = `import { Button } from '${componentSource}';

const view = <Button onClick={() => undefined}>保存</Button>;`;

    expect(sourceDiagnostics(source)).toEqual([]);
  });
});
