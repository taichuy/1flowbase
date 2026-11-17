const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const {
  parseUnit,
  unwrap,
  memberName,
  immutable,
  importOrigin,
  importedCall,
} = require("../source-unit.js");

const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
function createSourceGraph(repoRoot) {
  const units = new Map(),
    texts = new Map(),
    evidence = new Map(),
    exports = new Map();
  let steps = 0,
    bytes = 0;
  function tick() {
    if (++steps > 600000) throw new Error("HTTP proof budget exceeded");
  }
  function normalized(file) {
    file = path.posix.normalize(file);
    if (file.startsWith("../") || path.isAbsolute(file))
      throw new Error("source escaped repository");
    return file;
  }
  function loadText(file) {
    const absolute = path.join(repoRoot, normalized(file));
    if (
      !fs
        .realpathSync(absolute)
        .startsWith(fs.realpathSync(repoRoot) + path.sep)
    )
      throw new Error("source symlink escaped");
    const value = fs.readFileSync(absolute, "utf8");
    if (value.length > 512 * 1024)
      throw new Error("HTTP source budget exceeded");
    return value;
  }
  function text(file) {
    file = normalized(file);
    if (!texts.has(file)) {
      const value = loadText(file);
      if ((bytes += value.length) > 20 * 1024 * 1024)
        throw new Error("HTTP source budget exceeded");
      texts.set(file, value);
    }
    return texts.get(file);
  }
  function matchesText(file, needles) {
    tick();
    file = normalized(file);
    // Discovery is only a necessary lexical screen. Do not retain unrelated
    // repository text as proof; every candidate still goes through text/read.
    // Each probe retains the same path, symlink and per-file budget checks.
    const value = texts.get(file) ?? loadText(file);
    return needles.every((needle) => value.includes(needle));
  }
  function read(file) {
    if (!units.has(file)) {
      if (units.size >= 256) throw new Error("HTTP module budget exceeded");
      text(file);
      units.set(file, parseUnit(repoRoot, file));
    }
    return units.get(file);
  }
  function add(unit, node) {
    if (!unit || !node?.range) return;
    const spans = evidence.get(unit.file) ?? new Map();
    spans.set(node.range.join(":"), {
      start: node.range[0],
      end: node.range[1],
    });
    evidence.set(unit.file, spans);
  }
  function addText(file, start = 0, end = text(file).length) {
    const spans = evidence.get(file) ?? new Map();
    if (start < end) spans.set(`${start}:${end}`, { start, end });
    evidence.set(file, spans);
  }
  function files(directory, extension = /\.(?:ts|tsx)$/) {
    const result = [],
      pending = [directory];
    while (pending.length) {
      tick();
      const current = pending.pop();
      if (!fs.existsSync(path.join(repoRoot, current))) continue;
      for (const item of fs.readdirSync(path.join(repoRoot, current), {
        withFileTypes: true,
      })) {
        if (
          [
            "node_modules",
            "dist",
            "target",
            ".git",
            "_tests",
            "__tests__",
          ].includes(item.name)
        )
          continue;
        const file = `${current}/${item.name}`;
        if (item.isDirectory()) pending.push(file);
        else if (item.isFile() && extension.test(item.name)) result.push(file);
        if (result.length > 5000)
          throw new Error("HTTP inventory budget exceeded");
      }
    }
    return result;
  }
  function resolveFile(base) {
    const candidates = [
      base,
      ...[".ts", ".tsx", ".js", "/index.ts", "/index.tsx"].map(
        (ext) => base + ext,
      ),
    ].filter(
      (file) =>
        fs.existsSync(path.join(repoRoot, file)) &&
        fs.statSync(path.join(repoRoot, file)).isFile(),
    );
    return candidates.length === 1 ? candidates[0] : null;
  }
  let packages;
  function moduleFile(unit, module) {
    if (module.startsWith("."))
      return resolveFile(
        path.posix.normalize(
          path.posix.join(path.posix.dirname(unit.file), module),
        ),
      );
    packages ??= files("web/packages", /^package\.json$/).map((file) => {
      const manifest = JSON.parse(text(file));
      return { file, manifest };
    });
    const found = packages.filter((pkg) => pkg.manifest.name === module);
    if (found.length !== 1) return null;
    const pkg = found[0],
      entry = pkg.manifest.exports?.["."];
    const relative = typeof entry === "string" ? entry : entry?.default;
    if (typeof relative !== "string" || !relative.startsWith("./")) return null;
    addText(pkg.file);
    return resolveFile(path.posix.join(path.posix.dirname(pkg.file), relative));
  }
  function exported(file, name, seen = new Set()) {
    tick();
    const key = `${file}\0${name}`;
    if (exports.has(key)) return exports.get(key);
    if (seen.has(key) || seen.size > 128) return null;
    seen = new Set([...seen, key]);
    const source = text(file);
    // A plain identifier cannot be exported here without occurring in source;
    // star exports are resolved below, never assumed to select a known API.
    if (
      !source.includes(name) &&
      !source.includes("*") &&
      !source.includes("\\u")
    )
      return null;
    const unit = read(file),
      matches = [];
    for (const statement of unit.ast.body) {
      if (statement.type === "ExportNamedDeclaration") {
        const declaration = statement.declaration;
        if (
          declaration?.type === "FunctionDeclaration" &&
          declaration.id?.name === name
        )
          matches.push({ unit, node: declaration });
        if (declaration?.type === "VariableDeclaration")
          for (const item of declaration.declarations)
            if (item.id.type === "Identifier" && item.id.name === name)
              matches.push({ unit, node: item.id });
        for (const specifier of statement.specifiers)
          if ((specifier.exported.name ?? specifier.exported.value) === name) {
            const target = statement.source
              ? moduleFile(unit, statement.source.value)
              : null;
            const result = target
              ? exported(target, specifier.local.name, seen)
              : { unit, node: specifier.local };
            if (result) {
              add(unit, statement);
              matches.push(result);
            }
          }
      } else if (
        statement.type === "ExportAllDeclaration" &&
        !statement.exported
      ) {
        const target = moduleFile(unit, statement.source.value);
        const result = target && exported(target, name, seen);
        if (result) {
          add(unit, statement);
          matches.push(result);
        }
      }
    }
    const distinct = new Map(
      matches.map((result) => [
        `${result.unit.file}:${result.node.range[0]}`,
        result,
      ]),
    );
    const result = distinct.size === 1 ? [...distinct.values()][0] : null;
    exports.set(key, result);
    return result;
  }
  function reference(unit, expression, seen = new Set()) {
    expression = unwrap(expression);
    if (expression?.type !== "Identifier") return null;
    const variable = unit.binding(expression);
    if (!immutable(variable) || seen.has(variable)) return null;
    seen.add(variable);
    const definition = variable.defs[0];
    if (definition.type === "ImportBinding") {
      const origin = importOrigin(unit, expression),
        file = origin && moduleFile(unit, origin.module);
      if (!file || !origin || ["*", "default"].includes(origin.name))
        return null;
      const result = exported(file, origin.name);
      if (result) add(unit, unit.parents.get(definition.node));
      return result?.node.type === "Identifier"
        ? reference(result.unit, result.node, seen)
        : result;
    }
    if (definition.type === "FunctionName")
      return { unit, node: definition.node };
    if (
      definition.type === "Variable" &&
      definition.node.id.type === "Identifier"
    ) {
      const initializer = unwrap(definition.node.init);
      if (initializer?.type === "Identifier")
        return reference(unit, initializer, seen);
      return { unit, node: initializer, declaration: definition.node };
    }
    return null;
  }
  function speculate(resolve, accepts) {
    const priorEvidence = new Map(
      [...evidence].map(([file, spans]) => [
        file,
        new Map([...spans].map(([key, span]) => [key, { ...span }])),
      ]),
    );
    const priorExports = new Map(
      [...exports].map(([key, result]) => [key, result && { ...result }]),
    );
    let committed = false;
    try {
      const result = resolve();
      committed = Boolean(accepts(result));
      return committed ? result : null;
    } finally {
      if (!committed) {
        // Rejected discovery must not leave proof spans or cached export paths.
        // Keep parsed/text caches and all spent budgets; errors still propagate.
        evidence.clear();
        for (const [file, spans] of priorEvidence) evidence.set(file, spans);
        exports.clear();
        for (const [key, result] of priorExports) exports.set(key, result);
      }
    }
  }
  function fn(unit, expression) {
    expression = unwrap(expression);
    if (FUNCTIONS.has(expression?.type)) return { unit, fn: expression };
    const target = reference(unit, expression);
    return FUNCTIONS.has(target?.node?.type)
      ? { unit: target.unit, fn: target.node }
      : null;
  }
  function walk(unit, root, visit, skipFunctions = true) {
    const stack = [root];
    while (stack.length) {
      tick();
      const node = stack.pop();
      if (!node || (skipFunctions && node !== root && FUNCTIONS.has(node.type)))
        continue;
      if (visit(node) === false) continue;
      for (const key of unit.visitorKeys[node.type] || [])
        for (const child of Array.isArray(node[key]) ? node[key] : [node[key]])
          if (child?.type) stack.push(child);
    }
  }
  function owner(unit, node) {
    for (
      let parent = unit.parents.get(node);
      parent;
      parent = unit.parents.get(parent)
    )
      if (FUNCTIONS.has(parent.type)) return parent;
    return null;
  }
  const key = (unit, node) => `${unit.file}:${node?.range?.join(":")}`;
  function nativeClosed(unit) {
    for (const node of unit.nodes) {
      tick();
      if (node.type === "ImportExpression") return false;
      if (
        ["AssignmentExpression", "UpdateExpression"].includes(node.type) ||
        (node.type === "UnaryExpression" && node.operator === "delete")
      ) {
        let root = unwrap(node.left ?? node.argument);
        while (root?.type === "MemberExpression") root = unwrap(root.object);
        if (root?.type === "Identifier" && !unit.binding(root)?.defs.length)
          return false;
      }
      if (
        node.type === "Identifier" &&
        unit.bindings.has(node) &&
        !unit.binding(node)?.defs.length &&
        ["eval", "Function", "Reflect"].includes(node.name)
      )
        return false;
      if (
        node.type === "Identifier" &&
        unit.bindings.has(node) &&
        !unit.binding(node)?.defs.length &&
        ["globalThis", "window", "self"].includes(node.name)
      ) {
        const parent = unit.parents.get(node);
        if (parent?.type === "UnaryExpression" && parent.operator === "typeof")
          continue;
        if (
          parent?.type !== "MemberExpression" ||
          parent.object !== node ||
          parent.computed
        )
          return false;
      }
      if (
        node.type === "MemberExpression" &&
        ["fetch", "constructor", "defineProperty", "setPrototypeOf"].includes(
          memberName(node),
        )
      )
        return false;
      if (
        node.type === "Identifier" &&
        node.name === "fetch" &&
        unit.bindings.has(node)
      ) {
        const parent = unit.parents.get(node);
        if (
          unit.binding(node)?.defs.length ||
          parent?.type !== "CallExpression" ||
          parent.callee !== node
        )
          return false;
      }
    }
    return true;
  }
  return {
    repoRoot,
    read,
    text,
    matchesText,
    add,
    addText,
    files,
    resolveFile,
    moduleFile,
    exported,
    reference,
    speculate,
    fn,
    walk,
    owner,
    key,
    tick,
    nativeClosed,
    evidence: () =>
      [...evidence].map(([file, spans]) => ({
        file,
        sha256: createHash("sha256").update(text(file)).digest("hex"),
        spans: [...spans.values()],
      })),
    units,
  };
}
module.exports = {
  createSourceGraph,
  FUNCTIONS,
  unwrap,
  memberName,
  immutable,
  importedCall,
  importOrigin,
};
