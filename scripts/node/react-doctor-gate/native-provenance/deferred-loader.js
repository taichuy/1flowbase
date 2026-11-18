const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");
const {
  unwrap,
  memberName,
  immutable,
  importedCall,
} = require("../source-unit.js");

// One total code-unit map: owner + observed key helper + four real UI targets.
const MAX_CODE_UNITS = 6;
const MAX_ASSETS = 4;
const MAX_ASSET_BYTES = 64 * 1024;
const MAX_TOTAL_ASSET_BYTES = 128 * 1024;
const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
let postcss;
function cssParser() {
  // Resolve Vite's direct parser dependency, not a user config/plugin pipeline.
  const requireWeb = createRequire(
    path.resolve(__dirname, "../../../../web/package.json"),
  );
  const requireVite = createRequire(requireWeb.resolve("vite/package.json"));
  return (postcss ??= requireVite("postcss"));
}

function createDeferredLoaderAudit({
  unit,
  state,
  tick,
  add,
  auditTarget,
  depthLimit,
}) {
  const calls = new Map();
  const root = () => fs.realpathSync(state.repoRoot);
  function relative(source, specifier) {
    tick();
    if (
      typeof specifier !== "string" ||
      !/^\.\.?\//.test(specifier) ||
      /[?#\0]/.test(specifier)
    )
      return null;
    const file = path.posix.normalize(
      path.posix.join(path.posix.dirname(source.file), specifier),
    );
    return file.startsWith("web/app/") ? file : null;
  }
  function checked(file) {
    const absolute = path.join(state.repoRoot, file);
    const real = fs.realpathSync(absolute);
    const stat = fs.statSync(absolute);
    if (!real.startsWith(root() + path.sep) || !stat.isFile()) return null;
    return { absolute, real, size: stat.size };
  }
  function code(source, specifier) {
    const base = relative(source, specifier);
    if (
      !base ||
      (path.posix.extname(base) && !/\.(?:ts|tsx|js|jsx)$/.test(base))
    )
      return null;
    const candidates = [
      base,
      ...[".ts", ".tsx", ".js", ".jsx"].map((extension) => base + extension),
    ].filter((file) => {
      try {
        return fs.statSync(path.join(state.repoRoot, file)).isFile();
      } catch {
        return false;
      }
    });
    if (candidates.length !== 1 || !checked(candidates[0])) return null;
    const file = candidates[0];
    if (!state.units.has(file)) {
      if (state.units.size >= MAX_CODE_UNITS) return null;
      const target = state.read(file);
      if (!target) return null;
      state.units.set(file, target);
    }
    return state.units.get(file);
  }
  function asset(source, declaration) {
    const file = relative(source, declaration.source.value);
    if (!file || !file.endsWith(".css")) return false;
    const located = checked(file);
    if (!located) return false;
    let data = state.assets.get(located.real);
    if (!data) {
      if (
        state.assets.size >= MAX_ASSETS ||
        located.size > MAX_ASSET_BYTES ||
        state.assetBytes + located.size > MAX_TOTAL_ASSET_BYTES
      )
        return false;
      const bytes = fs.readFileSync(located.absolute);
      if (
        state.assets.size >= MAX_ASSETS ||
        bytes.length > MAX_ASSET_BYTES ||
        state.assetBytes + bytes.length > MAX_TOTAL_ASSET_BYTES
      )
        return false;
      const text = bytes.toString("utf8");
      if (!text.length || !Buffer.from(text, "utf8").equals(bytes))
        return false;
      const parsed = cssParser().parse(text, { from: located.absolute });
      tick();
      parsed.walk(() => tick());
      // Data evidence only. Do not put a CSS tree/object in the code-unit map.
      data = { file, source: text };
      state.assets.set(located.real, data);
      state.assetBytes += bytes.length;
      add(data, { range: [0, text.length] });
    }
    add(source, declaration);
    return true;
  }
  function moduleEffects(source) {
    for (const statement of source.ast.body) {
      tick();
      if (statement.type !== "ImportDeclaration" || statement.specifiers.length)
        continue;
      if (typeof statement.source.value !== "string") return false;
      if (statement.source.value.endsWith(".css")) {
        if (!asset(source, statement)) return false;
      } else {
        const target = code(source, statement.source.value);
        if (!target || !auditTarget(target)) return false;
        add(source, statement);
      }
    }
    return true;
  }
  function localComponent(source, node, seen = new Set(), depth = 0) {
    tick();
    node = unwrap(node);
    if (!node || depth > depthLimit) return false;
    if (FUNCTIONS.has(node.type)) {
      if (node.async || node.generator) return false;
      return !node.id || immutable(source.binding(node.id));
    }
    if (node.type !== "Identifier") return false;
    const variable = source.binding(node);
    if (!immutable(variable) || seen.has(variable)) return false;
    const definition = variable.defs[0];
    seen = new Set([...seen, variable]);
    if (definition.type === "FunctionName")
      return localComponent(source, definition.node, seen, depth + 1);
    if (
      definition.type !== "Variable" ||
      definition.node.id.type !== "Identifier"
    )
      return false;
    add(source, definition.node);
    return localComponent(source, definition.node.init, seen, depth + 1);
  }
  function selectedExport(source, name) {
    const matches = [];
    for (const statement of source.ast.body) {
      tick();
      if (
        name === "default" &&
        statement.type === "ExportDefaultDeclaration" &&
        localComponent(source, statement.declaration)
      )
        matches.push(statement);
      if (statement.type !== "ExportNamedDeclaration" || statement.source)
        continue;
      const declaration = statement.declaration;
      if (
        declaration?.type === "FunctionDeclaration" &&
        declaration.id?.name === name &&
        localComponent(source, declaration)
      )
        matches.push(statement);
      if (declaration?.type === "VariableDeclaration") {
        for (const item of declaration.declarations)
          if (
            item.id.type === "Identifier" &&
            item.id.name === name &&
            immutable(source.binding(item.id)) &&
            localComponent(source, item.init)
          )
            matches.push(statement);
      }
      for (const specifier of statement.specifiers ?? [])
        if (
          (specifier.exported.name ?? specifier.exported.value) === name &&
          localComponent(source, specifier.local)
        )
          matches.push(statement);
    }
    if (matches.length !== 1) return false;
    add(source, matches[0]);
    return true;
  }
  function returned(fn) {
    fn = unwrap(fn);
    if (!FUNCTIONS.has(fn?.type) || fn.async || fn.generator) return null;
    if (fn.body.type !== "BlockStatement") return unwrap(fn.body);
    return fn.body.body.length === 1 &&
      fn.body.body[0].type === "ReturnStatement"
      ? unwrap(fn.body.body[0].argument)
      : null;
  }
  function inspectLazy(call) {
    if (calls.has(call)) return calls.get(call);
    calls.set(call, false);
    tick();
    if (
      !importedCall(unit, call, "react", "lazy") ||
      call.optional ||
      call.arguments.length !== 1
    )
      return false;
    const loader = unwrap(call.arguments[0]);
    if (!FUNCTIONS.has(loader?.type) || loader.params.length) return false;
    let imported = returned(loader),
      selected = "default";
    if (imported?.type === "CallExpression") {
      const then = unwrap(imported.callee);
      if (
        imported.optional ||
        then?.type !== "MemberExpression" ||
        then.optional ||
        memberName(then) !== "then" ||
        imported.arguments.length !== 1
      )
        return false;
      const projection = unwrap(imported.arguments[0]),
        result = returned(projection);
      if (
        !projection ||
        projection.params.length !== 1 ||
        projection.params[0].type !== "Identifier" ||
        result?.type !== "ObjectExpression" ||
        result.properties.length !== 1
      )
        return false;
      const field = result.properties[0],
        selector = unwrap(field.value);
      if (
        field.type !== "Property" ||
        field.kind !== "init" ||
        field.method ||
        field.computed ||
        (field.key.name ?? field.key.value) !== "default" ||
        selector?.type !== "MemberExpression" ||
        selector.optional ||
        memberName(selector) === null ||
        unwrap(selector.object)?.type !== "Identifier" ||
        unit.binding(unwrap(selector.object)) !==
          unit.binding(projection.params[0])
      )
        return false;
      selected = memberName(selector);
      imported = unwrap(then.object);
    }
    if (
      imported?.type !== "ImportExpression" ||
      imported.source?.type !== "Literal" ||
      typeof imported.source.value !== "string" ||
      imported.options ||
      imported.attributes
    )
      return false;
    const declaration = unit.parents.get(call);
    const statement = unit.parents.get(declaration);
    if (
      declaration?.type !== "VariableDeclarator" ||
      declaration.init !== call ||
      declaration.id.type !== "Identifier" ||
      !immutable(unit.binding(declaration.id)) ||
      statement?.type !== "VariableDeclaration" ||
      statement.declarations.length !== 1
    )
      return false;
    const target = code(unit, imported.source.value);
    if (!target || !selectedExport(target, selected) || !auditTarget(target))
      return false;
    add(unit, statement);
    calls.set(call, true);
    return true;
  }
  function inspectImport(node) {
    let expression = node,
      parent = unit.parents.get(expression);
    if (
      parent?.type === "MemberExpression" &&
      parent.object === expression &&
      memberName(parent) === "then"
    ) {
      expression = unit.parents.get(parent);
      if (expression?.type !== "CallExpression") return false;
      parent = unit.parents.get(expression);
    }
    // Wrappers can be unwrapped without accepting another executable operation.
    while (parent?.expression === expression && unwrap(parent) === node) {
      expression = parent;
      parent = unit.parents.get(expression);
    }
    const loader =
      parent?.type === "ReturnStatement"
        ? unit.parents.get(unit.parents.get(parent))
        : parent;
    if (!FUNCTIONS.has(loader?.type)) return false;
    const call = unit.parents.get(loader);
    return call?.type === "CallExpression" && inspectLazy(call);
  }
  return { inspectLazy, inspectImport, moduleEffects };
}

module.exports = { createDeferredLoaderAudit, MAX_CODE_UNITS };
