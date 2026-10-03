import ts from 'typescript';
import { describe, expect, test } from 'vitest';

import { collectNativeModuleDeclarations } from '../../../../../../build/native-module-declarations';
import { FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS } from '../editor-declarations';

describe('frontend Monaco module declarations', () => {
  test('infinite scroll exposes real component props', () => {
    expect(
      typeCheckSource({
        extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
        source: `import type { Props } from 'react-infinite-scroll-component';
const props: Props = {dataLength: 0, next() {}, hasMore: true, loader: null, children: null, scrollableTarget: 'list'};
// @ts-expect-error dataLength must be numeric.
props.dataLength = 'invalid';
void props;`
      })
    ).toEqual([]);
  });

  test('I2242-AC-001 uses real StyleProvider and HappyProvider prop types', () => {
    expect(
      typeCheckSource({
        extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
        source: `import type { ComponentProps } from 'react';
import { StyleProvider } from '@ant-design/cssinjs';
import { HappyProvider } from '@ant-design/happy-work-theme';
const style: ComponentProps<typeof StyleProvider> = { hashPriority: 'high' };
const happy: ComponentProps<typeof HappyProvider> = { disabled: false };
// @ts-expect-error hashPriority must retain its literal union.
const invalidStyle: ComponentProps<typeof StyleProvider> = { hashPriority: 'invalid' };
// @ts-expect-error disabled is boolean, not any.
const invalidHappy: ComponentProps<typeof HappyProvider> = { disabled: 'yes' };
void style; void happy; void invalidStyle; void invalidHappy;`
      })
    ).toEqual([]);
  });

  test('AC-002 type-checks public Ant Design locales with their real Locale type', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import type { ConfigProviderProps } from 'antd';
import enUS from 'antd/locale/en_US';
import zhCN from 'antd/locale/zh_CN.js';
const locales: ConfigProviderProps['locale'][] = [enUS, zhCN];
// @ts-expect-error Locale has no arbitrary properties (must not become any).
enUS.nonexistentProperty();
void locales;`
    });
    expect(diagnostics).toEqual([]);
  });

  test('I2223 exposes the shared token trend contract to native blocks', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import { TokenTrendChart, type TokenTrendPoint } from '@1flowbase/token-trend';
const points: TokenTrendPoint[] = [{ bucket_start: '2026-10-03', input_tokens: 10, output_tokens: 2, input_cache_hit_tokens: 5, input_cache_hit_rate: 0.5, cache_write_tokens: null }];
const chart = <TokenTrendChart points={points} bucketLabels={['Oct 3']} labels={{cache_write_tokens: 'Cache write'}} />;
void chart;`
    });
    expect(diagnostics).toEqual([]);
  });

  test('AC-001/002 type-checks runtime and type-only exports from resolved dependencies', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import React from 'react';
import { Table } from 'antd';
import type { DividerProps, FlexProps, GetProp, TableProps } from 'antd';

const gap: FlexProps['gap'] = 'small';
type DividerClassNames = GetProp<DividerProps, 'classNames', 'Return'>;
interface DataType { key: string; name: string; }
const columns: TableProps<DataType>['columns'] = [{ dataIndex: 'name' }];
const App: React.FC = () => null;
const table = <Table<DataType> columns={columns} />;
void gap;
void App;
void table;
void (undefined as DividerClassNames);`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1929-AC-004 type-checks root and internal @dnd-kit imports from the generated inventory', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import type { DragEndEvent } from '@dnd-kit/core';
import { DndContext } from '@dnd-kit/core/dist/index.js';
import { arrayMove } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';

const event = undefined as DragEndEvent | undefined;
void event;
void DndContext;
void arrayMove;
void CSS;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1932-AC-003 type-checks @ant-design/colors from resolved package declarations', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import { cyan, generate, presetPalettes } from '@ant-design/colors';
import type { Palette } from '@ant-design/colors';

const generated: Palette = generate('#1677ff');
void cyan;
void generated;
void presetPalettes;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1945-AC-001 type-checks public @ant-design/icons leaf defaults', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import ClockCircleOutlined from '@ant-design/icons/ClockCircleOutlined';
import type { ComponentProps } from 'react';

const props: ComponentProps<typeof ClockCircleOutlined> = { spin: true };
const icon = <ClockCircleOutlined {...props} />;
void icon;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1933-AC-002 type-checks the dayjs default export and Dayjs type', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import dayjs from 'dayjs';
import type { Dayjs } from 'dayjs';

const start: Dayjs = dayjs('2026-01-01');
const output: string = start.add(1, 'day').format('YYYY-MM-DD');
void output;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1933-AC-004d type-checks dayjs plugins and runtime-only locales', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import dayjs from 'dayjs';
import utc from 'dayjs/plugin/utc';
import zhCn from 'dayjs/locale/zh-cn';

dayjs.extend(utc);
dayjs.locale(zhCn.name);
const output: string = dayjs.utc('2026-01-01').format('YYYY-MM-DD');
void output;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1951-AC-001 type-checks the lodash/debounce default export', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import debounce from 'lodash/debounce';

const debounced = debounce((value: string) => value, 100);
debounced.cancel();
debounced.flush();`
    });

    expect(diagnostics).toEqual([]);
  });

  test('I1952-AC-001/005 type-checks the clsx default and named exports', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import clsxDefault, { clsx as clsxNamed } from 'clsx';
import type { ClassValue } from 'clsx';

const input: ClassValue = { active: true, hidden: false };
const defaultResult: string = clsxDefault('base', input);
const namedResult: string = clsxNamed(['nested', input]);
void defaultResult;
void namedResult;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('D1-AC-001 type-checks the narrow BlockContext surface capability', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import type { BlockContext } from '@1flowbase/block-sdk';

declare const ctx: BlockContext;
declare const target: Element;
const accepted: boolean | undefined = ctx.ui.surface?.reveal(target);
void accepted;`
    });

    expect(diagnostics).toEqual([]);
  });

  test('D1-AC-004 exposes public forceAlign only for Tooltip-family refs', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import React from 'react';
import { Dropdown, Popover, Tooltip } from 'antd';

declare const popoverRef: React.ComponentRef<typeof Popover>;
declare const tooltipRef: React.ComponentRef<typeof Tooltip>;
declare const dropdownRef: React.ComponentRef<typeof Dropdown>;
popoverRef.forceAlign();
tooltipRef.forceAlign();
// @ts-expect-error Ant Design exposes only the Dropdown trigger HTMLElement.
dropdownRef.forceAlign();`
    });

    expect(diagnostics).toEqual([]);
  });

  test('rejects an invalid public prop after reusing the declaration program', () => {
    const diagnostics = typeCheckSource({
      extraLibs: FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
      source: `import type { FlexProps } from 'antd';
const gap: FlexProps['gap'] = { unsupported: true };
void gap;`
    });
    expect(diagnostics.join('\n')).toMatch(
      /not assignable|does not exist|known properties/
    );
  });

  test('AC-004 fails explicitly when a dependency declaration cannot resolve', () => {
    expect(() =>
      collectNativeModuleDeclarations({
        moduleSources: ['@1flowbase/definitely-missing-native-module'],
        projectRoot: process.cwd()
      })
    ).toThrow(/Cannot resolve declarations/);
  });
});

type DeclarationLibs = readonly {
  content: string;
  filePath: string;
  source: string;
}[];

type TypeCheckEnvironment = {
  files: Map<string, string>;
  host: ts.CompilerHost;
  options: ts.CompilerOptions;
  program?: ts.Program;
};

// All fixtures consume the same immutable declaration inventory. Reuse parsed
// libraries and TypeScript's incremental program, never a fixture's diagnostics.
const typeCheckEnvironments = new WeakMap<
  DeclarationLibs,
  TypeCheckEnvironment
>();
const sourcePath = '/demo.tsx';

function createTypeCheckEnvironment(
  extraLibs: DeclarationLibs
): TypeCheckEnvironment {
  const files = new Map<string, string>([[sourcePath, '']]);
  for (const extraLib of extraLibs) {
    files.set(new URL(extraLib.filePath).pathname, extraLib.content);
  }
  const directories = new Set<string>();
  for (const filePath of files.keys()) {
    for (
      let slash = filePath.lastIndexOf('/');
      slash >= 0;
      slash = filePath.lastIndexOf('/', slash - 1)
    ) {
      directories.add(filePath.slice(0, slash) || '/');
      if (slash === 0) break;
    }
  }
  const options: ts.CompilerOptions = {
    allowSyntheticDefaultImports: true,
    esModuleInterop: true,
    jsx: ts.JsxEmit.ReactJSX,
    lib: ['lib.es2022.d.ts'],
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Node10,
    noEmit: true,
    skipLibCheck: true,
    strict: true,
    types: []
  };
  const host = ts.createCompilerHost(options, true);
  const getSourceFile = host.getSourceFile.bind(host);
  const directoryExists = host.directoryExists?.bind(host);
  const parsedFiles = new Map<string, { text?: string; file: ts.SourceFile }>();
  host.fileExists = (filePath) =>
    files.has(filePath) || ts.sys.fileExists(filePath);
  host.readFile = (filePath) =>
    files.get(filePath) ?? ts.sys.readFile(filePath);
  host.directoryExists = (directoryPath) =>
    directories.has(directoryPath) || directoryExists?.(directoryPath) === true;
  host.getSourceFile = (filePath, languageVersion) => {
    const text = files.get(filePath);
    const cached = parsedFiles.get(filePath);
    if (cached && cached.text === text) return cached.file;
    const file =
      text !== undefined
        ? ts.createSourceFile(filePath, text, languageVersion, true)
        : getSourceFile(filePath, languageVersion);
    if (file) parsedFiles.set(filePath, { text, file });
    return file;
  };
  return { files, host, options };
}

function typeCheckSource({
  extraLibs,
  source
}: {
  extraLibs: DeclarationLibs;
  source: string;
}): string[] {
  let environment = typeCheckEnvironments.get(extraLibs);
  if (!environment) {
    environment = createTypeCheckEnvironment(extraLibs);
    typeCheckEnvironments.set(extraLibs, environment);
  }
  environment.files.set(sourcePath, source);
  environment.program = ts.createProgram(
    [sourcePath, ...[...environment.files.keys()].filter(isDeclarationFile)],
    environment.options,
    environment.host,
    environment.program
  );
  return ts
    .getPreEmitDiagnostics(environment.program)
    .filter(
      (diagnostic) =>
        !diagnostic.file || diagnostic.file.fileName === sourcePath
    )
    .map((diagnostic) =>
      ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ')
    );
}

function isDeclarationFile(filePath: string): boolean {
  return /\.d\.(?:c|m)?ts$/.test(filePath);
}
