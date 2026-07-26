export interface FrontstageJsxRequiredImport {
  kind: 'type' | 'value';
  name: string;
  moduleSource: string;
}

export type FrontstageJsxInsertion =
  | {
      kind: 'context-reference';
      memberPath: string;
    }
  | {
      kind: 'component';
      name: string;
      moduleSource: string;
      source: string;
    }
  | {
      kind: 'source';
      source: string;
      requiredImports?: readonly FrontstageJsxRequiredImport[];
    };

export interface FrontstageJsxSourceEdit {
  start: number;
  end: number;
  text: string;
}

export interface FrontstageJsxInsertionPlan {
  edits: FrontstageJsxSourceEdit[];
}

interface NamedImportDeclaration {
  start: number;
  end: number;
  kind: 'type' | 'value';
  moduleSource: string;
  quote: "'" | '"';
  specifiers: string[];
  multiline: boolean;
}

export function planFrontstageJsxInsertion({
  source,
  selection,
  insertion
}: {
  source: string;
  selection: { start: number; end: number };
  insertion: FrontstageJsxInsertion;
}): FrontstageJsxInsertionPlan {
  const requiredImports: FrontstageJsxRequiredImport[] = [];
  let insertedSource: string;

  switch (insertion.kind) {
    case 'context-reference':
      insertedSource = `${findComponentContextBinding(source)}.${insertion.memberPath}`;
      break;
    case 'component':
      insertedSource = insertion.source;
      requiredImports.push({
        kind: 'value',
        name: insertion.name,
        moduleSource: insertion.moduleSource
      });
      break;
    case 'source':
      insertedSource = insertion.source;
      requiredImports.push(...(insertion.requiredImports ?? []));
      break;
  }

  const edits: FrontstageJsxSourceEdit[] = [
    {
      start: selection.start,
      end: selection.end,
      text: insertedSource
    }
  ];
  const groupedImports = groupRequiredImports(requiredImports);
  for (const imports of groupedImports) {
    const importEdit = planRequiredImportEdit(source, imports);
    if (importEdit) edits.push(importEdit);
  }
  return { edits };
}

export function applyFrontstageJsxInsertionPlan(
  source: string,
  plan: FrontstageJsxInsertionPlan
): string {
  return [...plan.edits]
    .sort((left, right) => right.start - left.start)
    .reduce(
      (current, edit) =>
        `${current.slice(0, edit.start)}${edit.text}${current.slice(edit.end)}`,
      source
    );
}

function findComponentContextBinding(source: string): string {
  const componentBinding = source.match(
    /\b(?:export\s+default\s+)?function\s+[A-Za-z_$][A-Za-z0-9_$]*\s*\(\s*\{\s*ctx(?:\s*:\s*([A-Za-z_$][A-Za-z0-9_$]*))?/u
  );
  if (componentBinding) return componentBinding[1] ?? 'ctx';

  const arrowBinding = source.match(
    /\bexport\s+default\s+(?:async\s*)?\(\s*\{\s*ctx(?:\s*:\s*([A-Za-z_$][A-Za-z0-9_$]*))?/u
  );
  return arrowBinding?.[1] ?? 'ctx';
}

function groupRequiredImports(
  imports: readonly FrontstageJsxRequiredImport[]
): FrontstageJsxRequiredImport[][] {
  const groups = new Map<string, FrontstageJsxRequiredImport[]>();
  for (const requiredImport of imports) {
    const key = `${requiredImport.kind}:${requiredImport.moduleSource}`;
    const group = groups.get(key) ?? [];
    if (!group.some((item) => item.name === requiredImport.name)) {
      group.push(requiredImport);
    }
    groups.set(key, group);
  }
  return [...groups.values()];
}

function planRequiredImportEdit(
  source: string,
  requiredImports: readonly FrontstageJsxRequiredImport[]
): FrontstageJsxSourceEdit | null {
  const first = requiredImports[0];
  if (!first) return null;

  const declarations = readNamedImportDeclarations(source).filter(
    (declaration) => declaration.moduleSource === first.moduleSource
  );
  const missing = requiredImports.filter(
    (requiredImport) =>
      !declarations.some((declaration) =>
        declaration.specifiers.some(
          (specifier) => importedName(specifier) === requiredImport.name
        )
      )
  );
  if (missing.length === 0) return null;

  const target =
    declarations.find((declaration) => declaration.kind === first.kind) ??
    (first.kind === 'type'
      ? declarations.find((declaration) => declaration.kind === 'value')
      : undefined);
  if (!target) {
    const names = missing.map((item) => item.name).sort(localeCompare);
    return {
      start: 0,
      end: 0,
      text: `${renderNamedImport(first.kind, names, first.moduleSource, false)}\n\n`
    };
  }

  const addedSpecifiers = missing.map((item) =>
    target.kind === 'value' && item.kind === 'type'
      ? `type ${item.name}`
      : item.name
  );
  const specifiers = [...target.specifiers, ...addedSpecifiers].sort(
    (left, right) => localeCompare(importedName(left), importedName(right))
  );
  return {
    start: target.start,
    end: target.end,
    text: renderNamedImport(
      target.kind,
      specifiers,
      target.moduleSource,
      target.multiline,
      target.quote
    )
  };
}

function readNamedImportDeclarations(source: string): NamedImportDeclaration[] {
  const declarations: NamedImportDeclaration[] = [];
  const pattern =
    /(^|\n)(import\s+(type\s+)?\{([\s\S]*?)\}\s+from\s+(['"])([^'"\n]+)\5\s*;?)/g;
  for (const match of source.matchAll(pattern)) {
    const leadingNewline = match[1] ?? '';
    const declaration = match[2];
    const quote = match[5];
    const moduleSource = match[6];
    if (!declaration || !moduleSource || (quote !== "'" && quote !== '"')) {
      continue;
    }
    const start = (match.index ?? 0) + leadingNewline.length;
    declarations.push({
      start,
      end: start + declaration.length,
      kind: match[3] ? 'type' : 'value',
      moduleSource,
      quote,
      specifiers: (match[4] ?? '')
        .split(',')
        .map((specifier) => specifier.trim())
        .filter(Boolean),
      multiline: declaration.includes('\n')
    });
  }
  return declarations;
}

function renderNamedImport(
  kind: 'type' | 'value',
  specifiers: readonly string[],
  moduleSource: string,
  multiline: boolean,
  quote: "'" | '"' = "'"
): string {
  const prefix = kind === 'type' ? 'import type' : 'import';
  if (!multiline) {
    return `${prefix} { ${specifiers.join(', ')} } from ${quote}${moduleSource}${quote};`;
  }
  return `${prefix} {\n${specifiers
    .map(
      (specifier, index) =>
        `  ${specifier}${index < specifiers.length - 1 ? ',' : ''}`
    )
    .join('\n')}\n} from ${quote}${moduleSource}${quote};`;
}

function importedName(specifier: string): string {
  return (
    specifier
      .replace(/^type\s+/, '')
      .split(/\s+as\s+/)[0]
      ?.trim() ?? ''
  );
}

function localeCompare(left: string, right: string): number {
  return left.localeCompare(right);
}
