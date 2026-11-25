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
} = require("./source-unit.js");

// These are immutable release inputs, not component eligibility decisions.
// A contract is useful only after its export, props and rendered host are proved.
const RELEASES = {
  antd: {
    version: "6.7.1",
    integrity:
      "sha512-InlwUhDGkxOhlUYJAnzWHNLJrjQAuT2cVWIOFzaYAUW9Phr8qU5a0fknPWvAbQtrHCIWGF9LxZ9zO5OMwKoGOg==",
    files: {
      "package.json":
        "83b78ce8e7277ed11d62a96d626aab4ac12f024a169f4764cac51c580644a0ff",
      "es/index.js":
        "05030ed411e9836d97d98db8ef11d3a2cd0ae378e0fd8f8dc4d80f9dff21d248",
      "es/spin/index.js":
        "e2e231e07f647d581663337b34d178f0fc08a3a314544b1592c2e10a2b4ed794",
      "es/progress/index.js":
        "db3ead0575005dee37944cf6543e6376a6d691d5f9595e054a13faee33b43e7a",
      "es/progress/progress.js":
        "63d0ba3c70c59d3c2b9b3b9de42072a8c5ba5c5ec55baed6066242c0c0b79720",
      "es/skeleton/index.js":
        "a2579d88e1e4523afe9d1c6fb08bc097a958dad2fb8bfe7b67eb3501919b66e8",
      "es/skeleton/Skeleton.js":
        "df583d64fd02fd835820ce51c565a0814f3ab0fe6cde21f5fe76d35a69f11df4",
      "es/listy/index.js":
        "331b7f02be97a0f104088a1e1e36127fd88881cc8211de1e645f9a9f558b701e",
    },
  },
  "@rc-component/listy": {
    version: "1.4.0",
    integrity:
      "sha512-enNtH2D3nU00qqP+YFxG3V3rnaXpN6HecFUIc/Q5SjxFmg8Ab20os1odn90Bam5y+ZIiOh52HUqvu4HwZv8pkQ==",
    files: {
      "package.json":
        "934fc82f248493831ddc1fab51ad8bd26deeaffd798c91f8bc0a6d78b30fc4ad",
      "es/index.js":
        "1c5c29149e713001e3f3279b4fb5876b9f0b82a905f6fae75c62da990274a3be",
      "es/List.js":
        "fbb707733e47f1dca24aa3fa5fb7c1ea38964681851e0eced4d8bb3bd6b9b1ab",
      "es/VirtualList/index.js":
        "90caa6cac3d6d3095075a14f1ab16657b964cf4cd90b9de67d6909b120f5b0ab",
    },
  },
  "@rc-component/virtual-list": {
    version: "1.5.1",
    integrity:
      "sha512-boqHxdtyWC88u8quYgEO49bcBy5fzRiOcnBge+N4nLzs2k8hUQ/yw7JE9dM6yCBE4jSm5YSHVCVMS+suBuJGKA==",
    files: {
      "package.json":
        "236e78983f03aae0d6723692e5735ab59f6827b15eb8e575a0abceb6141c708a",
      "es/index.js":
        "483824975791ff5117c1504ce657d7d18573ba59dca97c1765a601b83a80199b",
      "es/List.js":
        "769b32e3c98818d50eec1ee9e0bcbf06b644330fa9c06f4a994e345219cafe6a",
      "es/Filler.js":
        "28155c41f6108ac8e498621adcb0c80c39002647082004f9ffae12680435ec49",
    },
  },
};
const UNKNOWN = Symbol("unproved render value");
const FUNCTIONS = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
]);
const hash = (value) => createHash("sha256").update(value).digest("hex");
const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function createRenderContracts(repoRoot, context) {
  const roots = new Map(),
    sources = new Map(),
    units = new Map();
  const step = () => {
    if (++context.steps > 40000)
      throw new Error("render contract proof budget exceeded");
  };
  const record = (file, source, node) =>
    context.evidence(file, source, node?.range ?? [0, source.length]);
  function packageRoot(name) {
    if (roots.has(name)) return roots.get(name);
    if (!RELEASES[name]) return null;
    let root = path.join(repoRoot, "web/app/node_modules", name);
    if (!fs.existsSync(root)) {
      const parent =
        name === "@rc-component/listy"
          ? packageRoot("antd")
          : packageRoot("@rc-component/listy");
      if (!parent) return null;
      const physical = fs.realpathSync(parent);
      const nodeModules =
        name === "@rc-component/listy"
          ? path.dirname(physical)
          : path.dirname(path.dirname(physical));
      root = path.join(nodeModules, name);
    }
    if (!fs.existsSync(root)) return null;
    const physical = fs.realpathSync(root),
      boundary = fs.realpathSync(repoRoot);
    if (!physical.startsWith(`${boundary}${path.sep}`)) return null;
    roots.set(name, root);
    const metadata = read(name, "package.json");
    if (!metadata) return null;
    const pkg = JSON.parse(metadata.source);
    if (pkg.name !== name || pkg.version !== RELEASES[name].version)
      throw new Error("unproved package version");
    const lockPath = "web/pnpm-lock.yaml";
    const lock = fs.readFileSync(path.join(repoRoot, lockPath), "utf8");
    if (lock.length > 2 * 1024 * 1024) throw new Error("lockfile proof bound");
    const entry = escapeRegExp(`${name}@${RELEASES[name].version}`);
    const integrity = escapeRegExp(RELEASES[name].integrity);
    if (
      !new RegExp(
        `^  ['"]?${entry}['"]?:\\r?\\n    resolution: \\{integrity: ${integrity}\\}`,
        "m",
      ).test(lock)
    )
      throw new Error("unproved installed package integrity");
    record(lockPath, lock);
    record(metadata.file, metadata.source);
    return root;
  }
  function read(name, relative) {
    step();
    relative = path.posix.normalize(relative);
    if (!RELEASES[name]?.files[relative]) return null;
    const key = `${name}/${relative}`;
    if (sources.has(key)) return sources.get(key);
    if (sources.size >= 24) throw new Error("render source count bound");
    const root = roots.get(name) ?? packageRoot(name);
    if (!root) return null;
    const file = path
      .relative(repoRoot, path.join(root, relative))
      .split(path.sep)
      .join("/");
    const source = fs.readFileSync(path.join(repoRoot, file), "utf8");
    if (
      source.length > 512 * 1024 ||
      hash(source) !== RELEASES[name].files[relative]
    )
      throw new Error("installed render artifact changed");
    const result = { name, relative, file, source };
    sources.set(key, result);
    record(file, source);
    return result;
  }
  function unitFor(name, relative) {
    const input = read(name, relative);
    if (!input) return null;
    const key = `${name}/${relative}`;
    if (!units.has(key)) units.set(key, parseUnit(repoRoot, input.file));
    const unit = units.get(key);
    return unit ? { ...input, unit } : null;
  }
  function modulePath(name, base, module) {
    if (module.startsWith(".")) {
      const stem = path.posix.normalize(
        path.posix.join(path.posix.dirname(base), module),
      );
      const candidates = [stem, `${stem}.js`, `${stem}/index.js`].filter(
        (file) => RELEASES[name]?.files[file],
      );
      return candidates.length === 1 ? { name, relative: candidates[0] } : null;
    }
    return RELEASES[module] ? { name: module, relative: "es/index.js" } : null;
  }
  function exported(name, relative, exportedName, seen = new Set()) {
    step();
    const key = `${name}/${relative}:${exportedName}`;
    if (seen.has(key) || seen.size >= 16 || !packageRoot(name)) return null;
    seen.add(key);
    const source = unitFor(name, relative);
    if (!source) return null;
    for (const declaration of source.unit.ast.body) {
      if (
        exportedName === "default" &&
        declaration.type === "ExportDefaultDeclaration"
      )
        return symbol(source, declaration.declaration, seen);
      if (declaration.type !== "ExportNamedDeclaration" || !declaration.source)
        continue;
      const specifier = declaration.specifiers.find(
        (entry) =>
          (entry.exported.name ?? entry.exported.value) === exportedName,
      );
      if (!specifier) continue;
      const target = modulePath(name, relative, declaration.source.value);
      return target
        ? exported(
            target.name,
            target.relative,
            specifier.local.name ?? specifier.local.value,
            seen,
          )
        : null;
    }
    return null;
  }
  function symbol(source, expression, seen = new Set()) {
    step();
    expression = unwrap(expression);
    if (FUNCTIONS.has(expression?.type)) return { source, fn: expression };
    if (
      expression?.type === "CallExpression" &&
      ["forwardRef", "memo"].some((name) =>
        importedCall(source.unit, expression, "react", name),
      )
    )
      return symbol(source, expression.arguments[0], seen);
    if (expression?.type !== "Identifier") return null;
    const variable = source.unit.binding(expression);
    if (!immutable(variable)) return null;
    const definition = variable.defs[0];
    if (definition.type === "FunctionName")
      return { source, fn: definition.node };
    if (definition.type === "Variable")
      return symbol(source, definition.node.init, seen);
    const origin = importOrigin(source.unit, expression);
    const target =
      origin && modulePath(source.name, source.relative, origin.module);
    return target
      ? exported(target.name, target.relative, origin.name, seen)
      : null;
  }
  function appOrigin(unit, tag) {
    if (tag?.type === "JSXIdentifier") {
      const variable = unit.binding(tag);
      return variable?.identifiers[0]
        ? importOrigin(unit, variable.identifiers[0])
        : null;
    }
    if (
      tag?.type === "JSXMemberExpression" &&
      tag.object.type === "JSXIdentifier"
    ) {
      const origin = appOrigin(unit, tag.object);
      return origin && ["*", "default"].includes(origin.name)
        ? { module: origin.module, name: tag.property.name }
        : null;
    }
    return null;
  }
  function jsxProps(unit, opening, children) {
    const values = new Map();
    for (const attribute of opening.attributes) {
      if (
        attribute.type !== "JSXAttribute" ||
        attribute.name.type !== "JSXIdentifier" ||
        values.has(attribute.name.name) ||
        ["as", "component"].includes(attribute.name.name)
      )
        return null;
      const value =
        attribute.value?.type === "JSXExpressionContainer"
          ? unwrap(attribute.value.expression)
          : attribute.value;
      values.set(
        attribute.name.name,
        !value
          ? true
          : value.type === "Literal"
            ? value.value
            : FUNCTIONS.has(value.type)
              ? { kind: "app-function", unit, fn: value, attribute }
              : UNKNOWN,
      );
    }
    if (children?.length === 1)
      values.set("children", { kind: "app-jsx", node: children[0], unit });
    else if (children?.length) values.set("children", UNKNOWN);
    return { kind: "props", values };
  }
  function bind(source, pattern, value, env) {
    if (!pattern) return;
    if (pattern.type === "Identifier") {
      env.set(source.unit.binding(pattern), value);
      return;
    }
    if (pattern.type === "AssignmentPattern") {
      bind(
        source,
        pattern.left,
        value === undefined ? evaluate(source, pattern.right, env) : value,
        env,
      );
      return;
    }
    if (pattern.type !== "ObjectPattern") return;
    const used = new Set();
    for (const property of pattern.properties) {
      if (property.type === "RestElement") {
        bind(
          source,
          property.argument,
          value?.kind === "props"
            ? {
                kind: "props",
                values: new Map(
                  [...value.values].filter(([key]) => !used.has(key)),
                ),
              }
            : UNKNOWN,
          env,
        );
        continue;
      }
      const key = property.computed
        ? null
        : (property.key.name ?? property.key.value);
      used.add(key);
      bind(
        source,
        property.value,
        value?.kind === "props" ? value.values.get(key) : UNKNOWN,
        env,
      );
    }
  }
  function evaluate(source, expression, env, depth = 0) {
    step();
    expression = unwrap(expression);
    if (!expression || depth > 24) return UNKNOWN;
    if (expression.type === "Literal") return expression.value;
    if (expression.type === "Identifier") {
      const variable = source.unit.binding(expression);
      if (env.has(variable)) return env.get(variable);
      if (!variable && expression.name === "undefined") return undefined;
      return UNKNOWN;
    }
    if (FUNCTIONS.has(expression.type))
      return { kind: "function", source, fn: expression, env: new Map(env) };
    if (expression.type === "MemberExpression") {
      const object = evaluate(source, expression.object, env, depth + 1);
      return object?.kind === "props"
        ? object.values.get(memberName(expression))
        : UNKNOWN;
    }
    if (expression.type === "UnaryExpression" && expression.operator === "!") {
      const value = evaluate(source, expression.argument, env, depth + 1);
      return value === UNKNOWN ? UNKNOWN : !value;
    }
    if (
      expression.type === "BinaryExpression" &&
      expression.operator === "in"
    ) {
      const object = evaluate(source, expression.right, env, depth + 1);
      const key = evaluate(source, expression.left, env, depth + 1);
      return object?.kind === "props" && typeof key === "string"
        ? object.values.has(key)
        : UNKNOWN;
    }
    if (expression.type === "LogicalExpression") {
      const left = evaluate(source, expression.left, env, depth + 1);
      if (left === UNKNOWN) return UNKNOWN;
      const chooseRight =
        expression.operator === "??"
          ? left === null || left === undefined
          : expression.operator === "||"
            ? !left
            : !!left;
      return chooseRight
        ? evaluate(source, expression.right, env, depth + 1)
        : left;
    }
    if (expression.type === "ConditionalExpression") {
      const test = evaluate(source, expression.test, env, depth + 1);
      return test === UNKNOWN
        ? UNKNOWN
        : evaluate(
            source,
            test ? expression.consequent : expression.alternate,
            env,
            depth + 1,
          );
    }
    if (expression.type === "ObjectExpression") {
      const values = new Map();
      for (const property of expression.properties) {
        if (property.type === "SpreadElement") {
          const spread = evaluate(source, property.argument, env, depth + 1);
          if (spread?.kind === "props")
            for (const [key, value] of spread.values) values.set(key, value);
          continue;
        }
        if (
          property.type !== "Property" ||
          property.computed ||
          property.kind !== "init"
        )
          return UNKNOWN;
        values.set(
          property.key.name ?? property.key.value,
          evaluate(source, property.value, env, depth + 1),
        );
      }
      return { kind: "props", values };
    }
    if (
      expression.type === "CallExpression" &&
      importedCall(source.unit, expression, "react", "createElement")
    ) {
      return {
        kind: "element",
        source,
        node: expression,
        tag: unwrap(expression.arguments[0]),
        props: expression.arguments[1]
          ? evaluate(source, expression.arguments[1], env, depth + 1)
          : { kind: "props", values: new Map() },
      };
    }
    return UNKNOWN;
  }
  function execute(source, statements, env) {
    const returns = [];
    for (const statement of statements) {
      step();
      if (statement.type === "VariableDeclaration") {
        for (const declaration of statement.declarations)
          bind(
            source,
            declaration.id,
            evaluate(source, declaration.init, env),
            env,
          );
      } else if (statement.type === "ReturnStatement")
        return {
          returns: [...returns, evaluate(source, statement.argument, env)],
          continues: false,
        };
      else if (statement.type === "IfStatement") {
        const test = evaluate(source, statement.test, env);
        const branch = (node) =>
          !node
            ? { returns: [], continues: true }
            : execute(
                source,
                node.type === "BlockStatement" ? node.body : [node],
                new Map(env),
              );
        const branches =
          test === UNKNOWN
            ? [branch(statement.consequent), branch(statement.alternate)]
            : [branch(test ? statement.consequent : statement.alternate)];
        returns.push(...branches.flatMap((result) => result.returns));
        if (branches.every((result) => !result.continues))
          return { returns, continues: false };
      }
    }
    return { returns, continues: true };
  }
  function rootsOf(target, props, trace, active = new Set(), depth = 0) {
    step();
    if (!target || depth > 12 || active.has(target.fn)) return null;
    const nextActive = new Set([...active, target.fn]);
    const env = new Map();
    bind(target.source, target.fn.params[0], props, env);
    const result =
      target.fn.body.type === "BlockStatement"
        ? execute(target.source, target.fn.body.body, env)
        : {
            returns: [evaluate(target.source, target.fn.body, env)],
            continues: false,
          };
    trace.push({ ...target, env });
    record(target.source.file, target.source.source, target.fn);
    if (result.continues || !result.returns.length) return null;
    const hosts = [];
    for (const value of result.returns) {
      if (value?.kind === "element") {
        if (
          value.tag?.type === "Literal" &&
          typeof value.tag.value === "string"
        ) {
          record(value.source.file, value.source.source, value.tag);
          hosts.push(value.tag.value);
        } else {
          const nested = symbol(value.source, value.tag);
          const roots = rootsOf(
            nested,
            value.props,
            trace,
            nextActive,
            depth + 1,
          );
          if (!roots) return null;
          hosts.push(...roots);
        }
      } else if (
        value?.kind === "app-jsx" &&
        value.node.type === "JSXElement" &&
        value.node.openingElement.name.type === "JSXIdentifier" &&
        /^[a-z]/.test(value.node.openingElement.name.name)
      )
        hosts.push(value.node.openingElement.name.name);
      else return null;
    }
    return hosts;
  }
  function inspected(unit, element) {
    const props = jsxProps(unit, element.openingElement, element.children);
    const origin = appOrigin(unit, element.openingElement.name);
    if (
      !props ||
      !origin ||
      !RELEASES[origin.module] ||
      ["*", "default"].includes(origin.name)
    )
      return null;
    const target = exported(origin.module, "es/index.js", origin.name);
    const trace = [];
    const hosts = rootsOf(target, props, trace);
    return hosts ? { hosts, trace, props, origin } : null;
  }
  function virtualSlot(unit, element) {
    const contract = inspected(unit, element);
    if (
      !contract ||
      contract.props.values.get("virtual") !== true ||
      !contract.hosts.every((tag) => tag === "div")
    )
      return null;
    // A renderer slot is established from actual parameter flow, not a name
    // heuristic such as render* or a product component's identity.
    let slot = null,
      virtualTrace = null;
    for (const entry of contract.trace) {
      if (entry.source.name === "@rc-component/virtual-list")
        virtualTrace = entry;
      for (const call of entry.source.unit.nodes) {
        if (call.type !== "CallExpression" || call.callee.type !== "Identifier")
          continue;
        const callback = entry.env.get(entry.source.unit.binding(call.callee));
        if (callback?.kind !== "app-function") continue;
        let parent = entry.source.unit.parents.get(call);
        while (parent && parent !== entry.fn) {
          if (
            parent.type === "CallExpression" &&
            importedCall(entry.source.unit, parent, "react", "createElement") &&
            unwrap(parent.arguments[0])?.type === "Literal" &&
            unwrap(parent.arguments[0]).value === "div" &&
            parent.arguments
              .slice(2)
              .some(
                (child) =>
                  call.range[0] >= child.range[0] &&
                  call.range[1] <= child.range[1],
              )
          ) {
            slot = callback;
            record(entry.source.file, entry.source.source, parent);
            break;
          }
          parent = entry.source.unit.parents.get(parent);
        }
      }
    }
    if (!slot || !virtualTrace) return null;
    // Follow additional pinned literal host renderers referenced by the virtual
    // root (the filler/holder path), preserving their artifact evidence.
    for (const node of virtualTrace.source.unit.nodes) {
      if (
        node.type !== "CallExpression" ||
        !importedCall(virtualTrace.source.unit, node, "react", "createElement")
      )
        continue;
      const tag = unwrap(node.arguments[0]);
      if (tag?.type !== "Identifier") continue;
      const nested = symbol(virtualTrace.source, tag);
      if (nested)
        rootsOf(nested, { kind: "props", values: new Map() }, contract.trace);
    }
    return { callback: slot.fn, attribute: slot.attribute, contract };
  }
  return { inspected, virtualSlot };
}

module.exports = { createRenderContracts };
