import ts from 'typescript';
import { describe, expect, test } from 'vitest';

import { createFrontendModuleExtraLib } from '../declarations';

describe('frontend Monaco module declarations', () => {
  test('AC-001 type-checks the React.FC and TableProps patterns used by TSX demos', () => {
    const react = createFrontendModuleExtraLib('react', ['useState']);
    const antd = createFrontendModuleExtraLib('antd', ['Table']);

    const diagnostics = typeCheckDemo({
      reactDeclarations: react.content,
      antdDeclarations: antd.content
    });

    expect(diagnostics).toEqual([]);
  });
});

function typeCheckDemo({
  antdDeclarations,
  reactDeclarations
}: {
  antdDeclarations: string;
  reactDeclarations: string;
}): string[] {
  const sourcePath = '/demo.tsx';
  const files = new Map([
    [
      sourcePath,
      `import React from 'react';
import { Table } from 'antd';
import type { TableProps } from 'antd';

interface DataType { key: string; name: string; }

const columns: TableProps<DataType>['columns'] = [{
  title: 'Name',
  dataIndex: 'name',
  key: 'name'
}];

const App: React.FC = () => <Table<DataType> columns={columns} />;
void App;`
    ],
    ['/node_modules/react/index.d.ts', reactDeclarations],
    ['/node_modules/antd/index.d.ts', antdDeclarations]
  ]);
  const options: ts.CompilerOptions = {
    allowSyntheticDefaultImports: true,
    esModuleInterop: true,
    jsx: ts.JsxEmit.Preserve,
    lib: ['lib.es2022.d.ts'],
    moduleResolution: ts.ModuleResolutionKind.Node10,
    noEmit: true,
    strict: true,
    types: []
  };
  const host = ts.createCompilerHost(options, true);
  const getSourceFile = host.getSourceFile.bind(host);
  host.fileExists = (filePath) => files.has(filePath) || ts.sys.fileExists(filePath);
  host.readFile = (filePath) => files.get(filePath) ?? ts.sys.readFile(filePath);
  host.getSourceFile = (filePath, languageVersion) =>
    files.has(filePath)
      ? ts.createSourceFile(filePath, files.get(filePath)!, languageVersion, true)
      : getSourceFile(filePath, languageVersion);
  host.resolveModuleNames = (moduleNames) =>
    moduleNames.map((moduleName) => ({
      extension: ts.Extension.Dts,
      resolvedFileName: `/node_modules/${moduleName}/index.d.ts`
    }));

  return ts
    .getPreEmitDiagnostics(ts.createProgram([sourcePath], options, host))
    .map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, ' '));
}
