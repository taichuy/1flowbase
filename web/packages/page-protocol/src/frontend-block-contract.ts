export const FRONTEND_BLOCK_RUNTIMES = ['native_react'] as const;

export const FRONTEND_BLOCK_CONTEXT_PRIMITIVES = [
  'text',
  'image',
  'link',
  'button',
  'rich_text',
  'data_record'
] as const;

export const FRONTEND_BLOCK_UI_CAPABILITIES = [
  'responsive',
  'configurable',
  'theming',
  'data_binding'
] as const;

export const FRONTEND_BLOCK_PACKAGE_BOUNDARIES = [
  'page-protocol',
  'page-runtime',
  'block-renderer',
  'block-sdk'
] as const;

export type FrontendBlockRuntime = (typeof FRONTEND_BLOCK_RUNTIMES)[number];
export type FrontendBlockContextPrimitive =
  (typeof FRONTEND_BLOCK_CONTEXT_PRIMITIVES)[number];
export type FrontendBlockUiCapability =
  (typeof FRONTEND_BLOCK_UI_CAPABILITIES)[number];
export type FrontendBlockPackageBoundary =
  (typeof FRONTEND_BLOCK_PACKAGE_BOUNDARIES)[number];

export interface FrontendBlockContextContract {
  primitives: FrontendBlockContextPrimitive[];
  input_schema: Record<string, unknown>;
}

export interface FrontendBlockPermissions {
  network: string;
  storage: string;
  secrets: string;
}

export const FRONTEND_BLOCK_CODE_MODULE_SOURCES = [
  '@1flowbase/block-sdk',
  '@1flowbase/native-components'
] as const;

export type FrontendBlockCodeModuleSource =
  (typeof FRONTEND_BLOCK_CODE_MODULE_SOURCES)[number];

export interface FrontendBlockCodeModule {
  source: FrontendBlockCodeModuleSource;
  type_declarations: string;
}

export interface FrontendBlockCodeContribution {
  code_template?: string | null;
  code_template_version?: string | null;
  code_template_language?: 'jsx' | 'tsx' | null;
  code_modules?: FrontendBlockCodeModule[];
}

export interface FrontendBlockMonacoExtraLib {
  source: FrontendBlockCodeModuleSource;
  filePath: string;
  content: string;
}

export interface FrontendBlockCodeCapabilities {
  template: {
    source: string;
    version: string;
    language: 'jsx' | 'tsx';
  } | null;
  allowedImports: FrontendBlockCodeModuleSource[];
  monacoExtraLibs: FrontendBlockMonacoExtraLib[];
}

export interface FrontendBlockCatalogEntry extends FrontendBlockCodeContribution {
  installation_id: string;
  provider_code: string;
  plugin_id: string;
  plugin_version: string;
  contribution_code: string;
  title: string;
  runtime: FrontendBlockRuntime;
  entry: string;
  context_contract: FrontendBlockContextContract;
  permissions: FrontendBlockPermissions;
  ui_capabilities: FrontendBlockUiCapability[];
}

export interface FrontendBlockManifestContribution extends FrontendBlockCodeContribution {
  contribution_code: string;
  title: string;
  runtime: FrontendBlockRuntime;
  entry: string;
  context_contract: FrontendBlockContextContract;
  permissions: FrontendBlockPermissions;
  ui_capabilities: FrontendBlockUiCapability[];
}

export function createFrontendBlockCodeCapabilities(
  contribution: FrontendBlockCodeContribution
): FrontendBlockCodeCapabilities {
  const modules = (contribution.code_modules ?? []).filter((codeModule) =>
    FRONTEND_BLOCK_CODE_MODULE_SOURCES.includes(codeModule.source)
  );
  return {
    template:
      contribution.code_template &&
      contribution.code_template_version &&
      contribution.code_template_language
        ? {
            source: contribution.code_template,
            version: contribution.code_template_version,
            language: contribution.code_template_language
          }
        : null,
    allowedImports: modules.map((codeModule) => codeModule.source),
    monacoExtraLibs: modules.map((codeModule) => ({
      source: codeModule.source,
      filePath: `file:///node_modules/${codeModule.source}/index.d.ts`,
      content: codeModule.type_declarations
    }))
  };
}

const frontendBlockRuntimeSet = new Set<string>(FRONTEND_BLOCK_RUNTIMES);
const frontendBlockContextPrimitiveSet = new Set<string>(
  FRONTEND_BLOCK_CONTEXT_PRIMITIVES
);
const frontendBlockUiCapabilitySet = new Set<string>(
  FRONTEND_BLOCK_UI_CAPABILITIES
);
const frontendBlockPackageBoundarySet = new Set<string>(
  FRONTEND_BLOCK_PACKAGE_BOUNDARIES
);

export function isFrontendBlockRuntime(
  value: unknown
): value is FrontendBlockRuntime {
  return isStringInSet(value, frontendBlockRuntimeSet);
}

export function isFrontendBlockContextPrimitive(
  value: unknown
): value is FrontendBlockContextPrimitive {
  return isStringInSet(value, frontendBlockContextPrimitiveSet);
}

export function isFrontendBlockUiCapability(
  value: unknown
): value is FrontendBlockUiCapability {
  return isStringInSet(value, frontendBlockUiCapabilitySet);
}

export function isFrontendBlockPackageBoundary(
  value: unknown
): value is FrontendBlockPackageBoundary {
  return isStringInSet(value, frontendBlockPackageBoundarySet);
}

export function normalizeFrontendBlockRuntime(
  value: unknown
): FrontendBlockRuntime | null {
  return isFrontendBlockRuntime(value) ? value : null;
}

export function normalizeFrontendBlockContextPrimitive(
  value: unknown
): FrontendBlockContextPrimitive | null {
  return isFrontendBlockContextPrimitive(value) ? value : null;
}

export function normalizeFrontendBlockUiCapability(
  value: unknown
): FrontendBlockUiCapability | null {
  return isFrontendBlockUiCapability(value) ? value : null;
}

export function normalizeFrontendBlockContextPrimitives(
  values: unknown
): FrontendBlockContextPrimitive[] {
  return normalizeUniqueValues(values, isFrontendBlockContextPrimitive);
}

export function normalizeFrontendBlockUiCapabilities(
  values: unknown
): FrontendBlockUiCapability[] {
  return normalizeUniqueValues(values, isFrontendBlockUiCapability);
}

function isStringInSet(
  value: unknown,
  allowedValues: ReadonlySet<string>
): value is string {
  return typeof value === 'string' && allowedValues.has(value);
}

function normalizeUniqueValues<Value extends string>(
  values: unknown,
  guard: (value: unknown) => value is Value
): Value[] {
  if (!Array.isArray(values)) {
    return [];
  }

  const seen = new Set<Value>();
  const normalized: Value[] = [];

  for (const value of values) {
    if (!guard(value) || seen.has(value)) {
      continue;
    }

    seen.add(value);
    normalized.push(value);
  }

  return normalized;
}
