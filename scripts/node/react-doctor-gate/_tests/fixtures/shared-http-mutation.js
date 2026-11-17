const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { parseUnit } = require("../../source-unit.js");
const {
  maskRustText,
  delimitedEnd,
} = require("../../../check-rust-backend/source.js");

const ROOT = path.resolve(__dirname, "../../../../..");
const CONTROLLER =
  "web/app/src/features/frontstage/components/data-blocks/table/use-table-config-controller.ts";
const CANONICAL_CONTRACT =
  "api/crates/api/kernel/src/contracts/routes/console_interface.rs";
const RUNTIME_POLICY = "api/crates/interface-runtime/src/registry.rs";
const ACTUAL_REQUIRED = [
  CONTROLLER,
  "web/app/src/features/frontstage/components/data-blocks/table/table-model.ts",
  "web/app/src/features/frontstage/components/data-blocks/table/TableBlock.tsx",
  "web/app/src/features/frontstage/lib/data-blocks/host-context.tsx",
  "web/app/src/features/frontstage/components/data-blocks/DataBlockPageProvider.tsx",
  "web/app/src/features/frontstage/hooks/use-frontstage-block-tree-mutations.ts",
  "web/app/src/features/frontstage/api/block-tree.ts",
  "web/packages/api-client/src/console/frontstage-block-tree/requests.ts",
  "web/packages/api-client/src/transport.ts",
  "api/crates/api/console-interaction/src/routes/frontstage/block_tree.rs",
  "api/crates/api/console-interaction/src/routes/frontstage/block_tree/interface.rs",
  "api/crates/api/console-interaction/src/routes/console_interface.rs",
  CANONICAL_CONTRACT,
  RUNTIME_POLICY,
];

function copyFile(f, file) {
  f.write(file, fs.readFileSync(path.join(ROOT, file), "utf8"));
}

function copyProductionTree(f, directory) {
  for (const entry of fs.readdirSync(path.join(ROOT, directory), {
    withFileTypes: true,
  })) {
    if (["_tests", "__tests__", "node_modules", "dist"].includes(entry.name))
      continue;
    const file = `${directory}/${entry.name}`;
    if (entry.isDirectory()) copyProductionTree(f, file);
    else if (entry.isFile() && /\.(?:tsx?|json)$/.test(entry.name))
      copyFile(f, file);
  }
}

function canonicalSource(f) {
  for (const file of [
    CANONICAL_CONTRACT,
    RUNTIME_POLICY,
    "api/crates/api/kernel/Cargo.toml",
    "api/crates/api/kernel/src/lib.rs",
    "api/crates/api/kernel/src/contracts/mod.rs",
    "api/crates/api/kernel/src/contracts/interaction_shared/mod.rs",
    "api/crates/api/kernel/src/slices/mod.rs",
    "api/crates/api/kernel/src/slices/routes/mod.rs",
    "api/Cargo.toml",
  ])
    copyFile(f, file);
}

function actualGraph(f) {
  for (const file of [
    "web/package.json",
    "web/pnpm-workspace.yaml",
    "web/app/package.json",
    "web/app/tsconfig.json",
    "web/packages/api-client/package.json",
    "api/crates/api/console-interaction/Cargo.toml",
    "api/crates/api/console-interaction/src/lib.rs",
    "api/crates/api/console-interaction/src/routes/console_interface.rs",
    "api/crates/api/console-interaction/src/routes/frontstage/block_tree.rs",
    "api/crates/api/console-interaction/src/routes/frontstage/block_tree/interface.rs",
  ])
    copyFile(f, file);
  copyProductionTree(f, "web/app/src/features/frontstage");
  copyProductionTree(f, "web/packages/api-client/src");
  canonicalSource(f);
  for (const file of ACTUAL_REQUIRED.filter(
    (name) => name.endsWith(".ts") || name.endsWith(".tsx"),
  ))
    assert.ok(
      parseUnit(f.repoRoot, file),
      `current source must parse: ${file}`,
    );
  return ACTUAL_REQUIRED;
}

// The fixture is a literal Rust contract/source grammar, not a claim that this
// disposable miniature is an independently built Rust workspace. Production
// contract and policy owners above are copied verbatim, never redefined here.
function assertRustTokens(source) {
  const code = maskRustText(source);
  for (let cursor = 0; cursor < code.length; cursor++) {
    if ("([{ ".trim().includes(code[cursor])) {
      const end = delimitedEnd(code, cursor);
      assert.notEqual(end, null, "Rust fixture must have balanced token trees");
      cursor = end - 1;
    } else {
      assert.ok(
        !")]}".includes(code[cursor]),
        "unmatched Rust fixture delimiter",
      );
    }
  }
}

function syntheticGraph(f, options = {}) {
  const renamedIdentifiers = {
    useOwner: "readEnvelope",
    OwnerContext: "EnvelopeContext",
    OwnerProvider: "EnvelopeProvider",
    useCandidateEditor: "useLedgerEditor",
    adapt: "shapeInstruction",
    persist: "storeCandidate",
    insert: "appendEntry",
    replace: "replaceEntry",
    transmit: "exchange",
    collectionPath: "resourceAddress",
    host: "envelope",
    action: "instruction",
    column: "field",
  };
  const rename = (source) =>
    options.renamed
      ? source.replace(
          /\b[A-Za-z_$][\w$]*\b/g,
          (name) => renamedIdentifiers[name] ?? name,
        )
      : source;
  const names = options.renamed
    ? {
        operation: "transform",
        store: "publish",
        collection: "ledgers",
        key: "account",
        id: "entry",
        package: "remote-beta",
      }
    : {
        operation: "prepare",
        store: "save",
        collection: "containers",
        key: "scope",
        id: "item",
        package: "remote-alpha",
      };
  const prefix = `web/packages/${names.package}`;
  const files = {
    context: "web/app/src/ownership/context.tsx",
    provider: "web/app/src/ownership/provider.tsx",
    helper: "web/app/src/ownership/forwarder.ts",
    requests: `${prefix}/src/requests.ts`,
    transport: `${prefix}/src/transport.ts`,
    declarations: "api/crates/api/fixture/src/routes/operations/interface.rs",
    route: "api/crates/api/fixture/src/routes/operations.rs",
    facade: "api/crates/api/fixture/src/routes/console_interface.rs",
  };
  const method = options.method ?? "POST";
  const mutating = options.mutating ?? true;
  const binding = `http.example.${names.collection}.create.v7`;
  const interfaceId = `example.${names.collection}.write`;
  const resource = `/api/console/${names.collection}/:${names.key}/records`;
  const optionsSource = `{ path: collectionPath(owner), method: ${JSON.stringify(method)}, body: input, baseUrl: '' }`;
  const caller = `import { useOwner } from './ownership/context';
import { adapt, persist } from './ownership/forwarder';
export function useCandidateEditor() {
  const host = useOwner();
  const commit = async (next) => {
    const candidate = await next();
    await persist(host, candidate);
    return candidate;
  };
  const save = (next) => commit(async () => {
    const actions = [];
    const row_actions = [];
    const columns = [];
    for (const action of next.actions)
      actions.push(await adapt(host, action));
    for (const action of next.row_actions)
      row_actions.push(await adapt(host, action));
    for (const column of next.columns)
      columns.push(column.action ? { ...column, action: await adapt(host, column.action) } : column);
    return { ...next, actions, row_actions, columns };
  });
  return { save };
}`;
  f.write(f.file, caller);
  f.write(
    files.context,
    `import { createContext, useContext } from 'react';
export const OwnerContext = createContext(null);
export function useOwner() { return useContext(OwnerContext); }
`,
  );
  f.write(
    files.helper,
    `export async function adapt(host, action) {
  if (!action.enabled) return action;
  return host.${names.operation}(action);
}
export async function persist(host, candidate) { await host.${names.store}(candidate); }
`,
  );
  f.write(
    files.provider,
    `import { useMutation } from '@tanstack/react-query';
import { OwnerContext } from './context';
import { insert, replace } from '@fixture/${names.package}';
export function OwnerProvider({ owner, identity, children }) {
  const creation = useMutation({ mutationFn: input => insert(owner, input) });
  const replacement = useMutation({ mutationFn: input => replace(owner, identity, input) });
  const host = {
    identity,
    async ${names.operation}(action) {
      if (!action.enabled) return action;
      const created = await creation.mutateAsync({ parent: identity, title: action.title });
      return { ...action, target: created.id };
    },
    async ${names.store}(candidate) { await replacement.mutateAsync(candidate); }
  };
  return <OwnerContext.Provider value={host}>{children}</OwnerContext.Provider>;
}
`,
  );
  f.write(
    files.requests,
    `import { transmit } from './transport';
function collectionPath(owner) { return \`/api/console/${names.collection}/\${encodeURIComponent(owner)}/records\`; }
export function insert(owner, input) { return transmit(${optionsSource}); }
export function replace(owner, id, input) {
  return transmit({ path: collectionPath(owner) + '/' + encodeURIComponent(id), method: 'PATCH', body: input, baseUrl: '' });
}
`,
  );
  f.write(
    files.transport,
    `export async function transmit({ path, method = 'GET', body, baseUrl = '' }) {
  const response = await fetch(\`\${baseUrl}\${path}\`, { method, body: JSON.stringify(body) });
  if (!response.ok) throw new Error('request failed');
  return response.json();
}
`,
  );
  f.write(
    `${prefix}/src/index.ts`,
    "export { insert, replace } from './requests';\n",
  );
  f.write(
    `${prefix}/package.json`,
    JSON.stringify({
      name: `@fixture/${names.package}`,
      private: true,
      type: "module",
      exports: { ".": { default: "./src/index.ts" } },
    }),
  );
  f.write(
    "web/package.json",
    JSON.stringify({ private: true, workspaces: ["app", "packages/*"] }),
  );
  f.write("web/pnpm-workspace.yaml", "packages:\n  - app\n  - packages/*\n");
  f.write(
    "web/app/package.json",
    JSON.stringify({ name: "@fixture/editor", private: true }),
  );
  canonicalSource(f);
  f.write(
    "api/crates/api/fixture/Cargo.toml",
    '[package]\nname="fixture-api"\nversion="0.0.0"\nedition="2024"\n[dependencies]\napi-host-kernel={path="../kernel"}\n',
  );
  f.write(
    "api/crates/api/fixture/src/lib.rs",
    "pub mod routes { pub mod console_interface; pub mod operations; }\n",
  );
  f.write(
    files.facade,
    "pub use api_host_kernel::routes::console_interface::*;\n",
  );
  const declaration = `ConsoleInterfaceDeclaration {
    interface_id: "${interfaceId}", binding_id: "${binding}",
    method: "${method}", path: "${resource}", mutating: ${mutating},
  }`;
  const patchDeclaration = `ConsoleInterfaceDeclaration {
    interface_id: "${interfaceId}.replace", binding_id: "http.example.${names.collection}.patch.v7",
    method: "PATCH", path: "${resource}/:${names.id}", mutating: true,
  }`;
  f.write(
    files.declarations,
    `use crate::routes::console_interface::{self, ConsoleInterfaceDeclaration, ConsoleInterfacePort};
use std::sync::Arc;
const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
  ${declaration},
  ${patchDeclaration},
];
pub fn compile_registry<I, O>(port: Arc<dyn ConsoleInterfacePort<I, O>>) {
  console_interface::compile_registry("fixture-owner", "fixture-graph", DECLARATIONS, port);
}
`,
  );
  f.write(
    files.route,
    `use std::sync::Arc;
pub mod interface;
use crate::{ApiState, ApiError};
use axum::{http::HeaderMap, extract::{Path, State, Json}};
async fn invoke_operations(state: Arc<ApiState>, headers: HeaderMap, binding_id: &'static str, input: Request, mutating: bool) {
  let credential = if mutating {
    crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
  } else {
    crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
  };
  crate::routes::console_interface::invoke(state, binding_id, credential, input).await;
}
pub fn routes() {
  ConsoleRouteAssembly::new().route("/${names.collection}/:${names.key}/records", console_${method.toLowerCase()}(create_record));
}
#[utoipa::path(${method.toLowerCase()}, path = "${resource}")]
pub async fn create_record(State(state): State<Arc<ApiState>>, headers: HeaderMap, Path(owner): Path<String>, Json(body): Json<Request>) {
  invoke_operations(state, headers, "${binding}", body, ${options.csrf ?? mutating}).await;
}
`,
  );
  if (options.renamed)
    for (const [file, source] of [...f.sources])
      if (/\.(?:ts|tsx)$/.test(file)) f.write(file, rename(source));
  for (const [file, source] of f.sources) {
    if (/\.(?:ts|tsx)$/.test(file))
      assert.ok(
        parseUnit(f.repoRoot, file),
        `synthetic source must parse: ${file}`,
      );
    if (file.endsWith(".rs")) assertRustTokens(source);
  }
  return {
    files,
    names,
    binding,
    interfaceId,
    resource,
    declaration,
    anchor: rename("await adapt(host, action)"),
    columnAnchor: rename("await adapt(host, column.action)"),
    required: [
      f.file,
      ...Object.values(files),
      CANONICAL_CONTRACT,
      RUNTIME_POLICY,
    ],
  };
}

module.exports = {
  ROOT,
  CONTROLLER,
  CANONICAL_CONTRACT,
  RUNTIME_POLICY,
  actualGraph,
  syntheticGraph,
  assertRustTokens,
};
