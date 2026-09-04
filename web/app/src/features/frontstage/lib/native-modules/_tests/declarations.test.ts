import ts from 'typescript';
import { describe, expect, test } from 'vitest';

import { collectNativeModuleDeclarations } from '../../../../../../build/native-module-declarations';
import { FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS } from '../editor-declarations';

describe('frontend Monaco module declarations', () => {
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

  test('AC-004 fails explicitly when a dependency declaration cannot resolve', () => {
    expect(() =>
      collectNativeModuleDeclarations({
        moduleSources: ['@1flowbase/definitely-missing-native-module'],
        projectRoot: process.cwd()
      })
    ).toThrow(/Cannot resolve declarations/);
  });
});

function typeCheckSource({
  extraLibs,
  source
}: {
  extraLibs: readonly {
    content: string;
    filePath: string;
    source: string;
  }[];
  source: string;
}): string[] {
  const sourcePath = '/demo.tsx';
  const files = new Map<string, string>([[sourcePath, source]]);
  for (const extraLib of extraLibs) {
    files.set(new URL(extraLib.filePath).pathname, extraLib.content);
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
  host.fileExists = (filePath) =>
    files.has(filePath) || ts.sys.fileExists(filePath);
  host.readFile = (filePath) =>
    files.get(filePath) ?? ts.sys.readFile(filePath);
  host.directoryExists = (directoryPath) =>
    [...files.keys()].some((filePath) =>
      filePath.startsWith(`${directoryPath}/`)
    ) || directoryExists?.(directoryPath) === true;
  host.getSourceFile = (filePath, languageVersion) =>
    files.has(filePath)
      ? ts.createSourceFile(
          filePath,
          files.get(filePath)!,
          languageVersion,
          true
        )
      : getSourceFile(filePath, languageVersion);
  return ts
    .getPreEmitDiagnostics(
      ts.createProgram(
        [
          sourcePath,
          ...[...files.keys()].filter((filePath) => isDeclarationFile(filePath))
        ],
        options,
        host
      )
    )
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
