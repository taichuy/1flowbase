const {
  createSourceGraph,
  FUNCTIONS,
  unwrap,
  memberName,
  immutable,
  importedCall,
} = require("./source-graph.js");
const { finiteValueProof } = require("../source-unit.js");
const { createConsoleContracts } = require("./console-contract.js");

const MESSAGE =
  "This makes the for…of loop slow because each await runs one after another, so collect the independent calls & run them together with `await Promise.all(items.map(...))`";

function createSharedHttpMutationProof(repoRoot) {
  return (diagnostic, unit, awaited) => {
    if (
      diagnostic.rule !== "async-await-in-loop" ||
      diagnostic.message !== MESSAGE ||
      awaited.type !== "AwaitExpression"
    )
      return null;
    const graph = createSourceGraph(repoRoot),
      contracts = createConsoleContracts(graph);
    const resolving = new Set(),
      tracing = new Set();
    let appFiles,
      valueSteps = 0,
      traceCount = 0;
    const atom = (id, flags = {}) => ({ kind: "atom", id, ...flags });
    const literal = (value) => ({ kind: "literal", value });
    const flags = (values) => ({
      iteration: values.some((value) => value?.iteration),
      candidate: values.some((value) => value?.candidate),
      uncertain: values.some((value) => value?.uncertain),
    });
    function key(value) {
      if (!value) return "unknown";
      if (value.kind === "literal") return JSON.stringify(value.value);
      if (value.kind === "object")
        return `object:${graph.key(value.unit, value.node)}`;
      if (value.kind === "string")
        return value.parts
          .map((item) => (typeof item === "string" ? item : `{${key(item)}}`))
          .join("");
      return value.id ?? value.kind;
    }
    function property(value, name) {
      if (!value) return atom("unknown-property");
      if (value.kind === "unknown")
        return { kind: "unknown", id: `${value.id}.${name}`, uncertain: true };
      if (value.kind === "object")
        return (
          value.fields.get(name) ??
          (value.opaque
            ? atom(`${key(value)}.${name}`, flags([value]))
            : literal(undefined))
        );
      if (value.kind === "array" && /^\d+$/.test(name))
        return value.items[Number(name)] ?? atom("unknown-index");
      return atom(`${key(value)}.${name}`, flags([value]));
    }
    function fnValue(source, fn, env) {
      return {
        kind: "function",
        unit: source,
        fn,
        env: new Map(env),
        id: graph.key(source, fn),
      };
    }
    function bind(target, args) {
      const env = new Map(target.env);
      function pattern(node, input) {
        node = unwrap(node);
        if (!node) return;
        if (node.type === "Identifier")
          env.set(
            target.unit.binding(node),
            input ?? atom(graph.key(target.unit, node)),
          );
        else if (node.type === "AssignmentPattern")
          pattern(
            node.left,
            input?.kind === "literal" && input.value === undefined
              ? value(target.unit, node.right, env)
              : input,
          );
        else if (node.type === "ObjectPattern")
          for (const item of node.properties) {
            if (input?.kind === "object" && input.opaque)
              throw new Error("opaque destructured options");
            if (item.type !== "Property" || item.computed)
              throw new Error("opaque parameter pattern");
            pattern(
              item.value,
              property(input, item.key.name ?? item.key.value),
            );
          }
      }
      target.fn.params.forEach((parameter, index) =>
        pattern(parameter, args[index] ?? literal(undefined)),
      );
      return env;
    }
    function descendants(source, root, predicate) {
      const result = [];
      graph.walk(source, root, (node) => {
        if (predicate(node)) result.push(node);
      });
      return result;
    }
    function functionResult(target, args, depth) {
      if (depth > 20)
        return {
          kind: "unknown",
          id: `deep-call:${target.id}(${args.map(key).join(",")})`,
          uncertain: true,
        };
      const env = bind(target, args),
        returns =
          target.fn.body.type === "BlockStatement"
            ? descendants(
                target.unit,
                target.fn.body,
                (node) => node.type === "ReturnStatement",
              )
                .map((node) => node.argument)
                .filter(Boolean)
            : [target.fn.body];
      graph.add(target.unit, target.fn);
      if (returns.length === 1)
        return value(target.unit, returns[0], env, depth + 1);
      const results = returns.map((node) =>
        value(target.unit, node, env, depth + 1),
      );
      if (
        results.length &&
        results.every(
          (item) => item.kind === "literal" && item.value === results[0].value,
        )
      )
        return results[0];
      return atom(
        `result:${target.id}(${args.map(key).join(",")})`,
        flags(args),
      );
    }
    function contextValue(source, expression) {
      const target = graph.reference(source, expression);
      if (
        !target ||
        !importedCall(target.unit, target.node, "react", "createContext")
      )
        return null;
      const identity = graph.key(target.unit, target.node),
        providers = [];
      appFiles ??= graph.files("web/app/src");
      graph.add(target.unit, target.node);
      for (const file of appFiles) {
        if (!graph.text(file).includes("Provider")) continue;
        const candidate = graph.read(file);
        for (const opening of candidate.nodes.filter(
          (node) =>
            node.type === "JSXOpeningElement" &&
            node.name.type === "JSXMemberExpression" &&
            node.name.property.name === "Provider",
        )) {
          if (opening.name.object.type !== "JSXIdentifier") continue;
          const imported = candidate.nodes.find(
            (node) =>
              node.type === "Identifier" &&
              node.name === opening.name.object.name &&
              candidate.binding(node),
          );
          if (!imported) continue;
          const providerContext = graph.reference(candidate, imported);
          if (
            !providerContext ||
            graph.key(providerContext.unit, providerContext.node) !== identity
          )
            continue;
          const attribute = opening.attributes.find(
            (item) =>
              item.type === "JSXAttribute" && item.name.name === "value",
          );
          if (
            attribute?.value?.type !== "JSXExpressionContainer" ||
            opening.attributes.some((item) => item.type !== "JSXAttribute")
          )
            return null;
          const provided = value(
            candidate,
            attribute.value.expression,
            new Map(),
            1,
          );
          if (provided.kind !== "object") return null;
          provided.provider = identity;
          graph.add(candidate, opening);
          graph.add(candidate, provided.node);
          providers.push(provided);
        }
      }
      return providers.length === 1 ? providers[0] : null;
    }
    function externalParameter(source, variable, depth) {
      const definition = variable.defs[0],
        fn = FUNCTIONS.has(definition.node?.type)
          ? definition.node
          : graph.owner(source, definition.node);
      if (!fn?.id?.name) return null;
      const index = fn.params.findIndex(
        (parameter) =>
          unwrap(parameter)?.type === "Identifier" &&
          source.binding(unwrap(parameter)) === variable,
      );
      if (index < 0) return null;
      appFiles ??= graph.files("web/app/src");
      const matches = [];
      for (const file of appFiles) {
        if (!graph.text(file).includes(fn.id.name)) continue;
        const caller = graph.read(file);
        for (const call of caller.nodes.filter(
          (node) =>
            node.type === "CallExpression" &&
            unwrap(node.callee)?.type === "Identifier",
        )) {
          const target = graph.fn(caller, call.callee);
          if (
            !target ||
            target.unit.file !== source.file ||
            target.fn.range[0] !== fn.range[0]
          )
            continue;
          graph.add(caller, call);
          matches.push(
            value(caller, call.arguments[index], new Map(), depth + 1),
          );
        }
      }
      const identities = new Map(
        matches.map((result) => [key(result), result]),
      );
      return identities.size === 1 ? [...identities.values()][0] : null;
    }
    function value(source, expression, env, depth = 0) {
      graph.tick();
      if (++valueSteps > 60000) throw new Error("HTTP value budget exceeded");
      expression = unwrap(expression);
      // A deep, unrelated field can stay unknown without inventing data flow.
      // Consumers still require positive method, URI, owner and branch evidence.
      if (depth > 24)
        return {
          kind: "unknown",
          id: `deep-value:${graph.key(source, expression)}`,
          uncertain: true,
        };
      if (!expression) return literal(undefined);
      if (expression.type === "Literal") return literal(expression.value);
      if (expression.type === "Identifier") {
        const variable = source.binding(expression);
        if (env.has(variable)) return env.get(variable);
        if (!variable?.defs.length)
          return expression.name === "undefined"
            ? literal(undefined)
            : atom(`global:${expression.name}`);
        const id = `${source.file}:${variable.identifiers[0]?.range[0]}`;
        if (resolving.has(variable)) return atom(id);
        resolving.add(variable);
        let result;
        const definition = variable.defs[0];
        if (definition.type === "Parameter")
          result = externalParameter(source, variable, depth) ?? atom(id);
        else if (definition.type === "FunctionName")
          result = fnValue(source, definition.node, env);
        else if (definition.type === "Variable" && immutable(variable)) {
          const declaration = definition.node;
          if (declaration.id.type === "Identifier")
            result = value(source, declaration.init, env, depth + 1);
          else if (declaration.id.type === "ObjectPattern") {
            const field = declaration.id.properties.find(
              (item) =>
                item.type === "Property" &&
                item.value.type === "Identifier" &&
                source.binding(item.value) === variable,
            );
            result =
              field &&
              property(
                value(source, declaration.init, env, depth + 1),
                field.key.name ?? field.key.value,
              );
          }
        } else if (definition.type === "ImportBinding") {
          const target = graph.reference(source, expression);
          if (target)
            result = FUNCTIONS.has(target.node.type)
              ? fnValue(target.unit, target.node, new Map())
              : value(target.unit, target.node, new Map(), depth + 1);
        }
        resolving.delete(variable);
        return result ?? atom(id);
      }
      if (FUNCTIONS.has(expression.type))
        return fnValue(source, expression, env);
      if (expression.type === "AwaitExpression")
        return value(source, expression.argument, env, depth + 1);
      if (expression.type === "MemberExpression") {
        const name = memberName(expression);
        return name === null
          ? atom(`computed:${graph.key(source, expression)}`)
          : property(
              value(source, expression.object, env, depth + 1),
              String(name),
            );
      }
      if (expression.type === "ObjectExpression") {
        const fields = new Map(),
          values = [];
        let opaque = false;
        for (const item of expression.properties) {
          if (
            item.type !== "Property" ||
            item.kind !== "init" ||
            item.computed
          ) {
            opaque = true;
            continue;
          }
          const name = item.key.name ?? item.key.value;
          if (fields.has(name)) opaque = true;
          const field = value(source, item.value, env, depth + 1);
          fields.set(name, field);
          values.push(field);
        }
        return {
          kind: "object",
          fields,
          opaque,
          unit: source,
          node: expression,
          env,
          ...flags(values),
        };
      }
      if (expression.type === "ArrayExpression") {
        const items = expression.elements.map((item) =>
          value(source, item, env, depth + 1),
        );
        return { kind: "array", items, ...flags(items) };
      }
      if (expression.type === "TemplateLiteral") {
        const parts = [];
        expression.quasis.forEach((quasi, index) => {
          parts.push(quasi.value.cooked);
          if (index < expression.expressions.length) {
            const part = value(
              source,
              expression.expressions[index],
              env,
              depth + 1,
            );
            if (part.kind === "literal") parts.push(String(part.value));
            else if (part.kind === "string") parts.push(...part.parts);
            else parts.push(part);
          }
        });
        return {
          kind: "string",
          parts,
          ...flags(parts.filter((item) => typeof item !== "string")),
        };
      }
      if (
        expression.type === "BinaryExpression" &&
        expression.operator === "+"
      ) {
        const a = value(source, expression.left, env, depth + 1),
          b = value(source, expression.right, env, depth + 1);
        return {
          kind: "string",
          parts: [a, b].flatMap((item) =>
            item.kind === "string"
              ? item.parts
              : item.kind === "literal"
                ? [String(item.value)]
                : [item],
          ),
          ...flags([a, b]),
        };
      }
      if (
        expression.type === "ConditionalExpression" ||
        expression.type === "LogicalExpression"
      ) {
        const children =
          expression.type === "ConditionalExpression"
            ? [expression.consequent, expression.alternate]
            : [expression.left, expression.right];
        const values = children.map((child) =>
          value(source, child, env, depth + 1),
        );
        if (
          values.every(
            (item) => item.kind === "literal" && item.value === values[0].value,
          )
        )
          return values[0];
        return atom(
          `choice:${graph.key(source, expression)}:${values.map(key).join("|")}`,
          flags(values),
        );
      }
      if (expression.type === "CallExpression") {
        const callee = unwrap(expression.callee);
        if (
          callee?.type === "Identifier" &&
          callee.name === "encodeURIComponent" &&
          !source.binding(callee)?.defs.length
        )
          return value(source, expression.arguments[0], env, depth + 1);
        if (importedCall(source, expression, "react", "useContext"))
          return (
            contextValue(source, expression.arguments[0]) ??
            atom("unresolved-context")
          );
        if (importedCall(source, expression, "react", "useCallback"))
          return value(source, expression.arguments[0], env, depth + 1);
        if (importedCall(source, expression, "react", "useMemo")) {
          const fn = value(source, expression.arguments[0], env, depth + 1);
          return fn.kind === "function"
            ? functionResult(fn, [], depth + 1)
            : atom("opaque-memo");
        }
        if (
          importedCall(
            source,
            expression,
            "@tanstack/react-query",
            "useMutation",
          )
        ) {
          const options = value(
            source,
            expression.arguments[0],
            env,
            depth + 1,
          );
          graph.add(source, expression);
          return options.kind === "object" &&
            !options.opaque &&
            options.fields.get("mutationFn")?.kind === "function"
            ? {
                kind: "mutation",
                fn: options.fields.get("mutationFn"),
                id: graph.key(source, expression),
              }
            : atom("opaque-mutation");
        }
        const target = value(source, callee, env, depth + 1),
          args = expression.arguments.map((arg) =>
            value(source, arg, env, depth + 1),
          );
        if (target.kind === "function")
          return functionResult(target, args, depth + 1);
        return atom(
          `call:${graph.key(source, callee)}(${args.map(key).join(",")})`,
          flags(args),
        );
      }
      return atom(graph.key(source, expression));
    }
    function possible(source, test, env, desired, constraints) {
      const model = finiteValueProof({ steps: 0 });
      const terms = [...constraints, { source, test, env, desired }].map(
        (condition) => ({
          desired: condition.desired,
          term: model.expression(condition.source, condition.test, (node) => {
            if (
              !["Identifier", "MemberExpression", "CallExpression"].includes(
                node.type,
              )
            )
              return undefined;
            const result = value(condition.source, node, condition.env);
            if (result.uncertain) return null;
            return result.kind === "literal"
              ? model.constant(result.value)
              : model.input(key(result), [false, true]);
          }),
        }),
      );
      return (
        terms.every((item) => item.term) &&
        model.witness(
          terms.map((item) => item.term),
          (values) =>
            values.every(
              (item, index) => Boolean(item) === terms[index].desired,
            ),
        )
      );
    }
    function request(source, call, env, requests, provider, conditions) {
      const callee = unwrap(call.callee);
      if (callee?.type === "Identifier" && callee.name === "fetch") {
        if (
          source.binding(callee)?.defs.length ||
          !graph.nativeClosed(source) ||
          call.arguments.length !== 2
        )
          return;
        if (unwrap(call.arguments[1])?.type !== "ObjectExpression") return;
        const options = value(source, call.arguments[1], env),
          url = value(source, call.arguments[0], env);
        if (options.kind !== "object" || options.opaque) return;
        const method = options.fields.get("method"),
          bodyNode = call.arguments[1].properties.find(
            (item) =>
              item.type === "Property" &&
              (item.key.name ?? item.key.value) === "body",
          );
        if (
          method?.kind !== "literal" ||
          !["POST", "PATCH"].includes(method.value) ||
          !bodyNode
        )
          return;
        let body = bodyNode.value;
        if (body.type === "ConditionalExpression") body = body.consequent;
        if (
          body.type === "CallExpression" &&
          memberName(body.callee) === "stringify" &&
          unwrap(body.callee.object)?.name === "JSON" &&
          !source.binding(unwrap(body.callee.object))?.defs.length
        )
          body = body.arguments[0];
        const payload = value(source, body, env);
        graph.add(source, call);
        requests.push({ method: method.value, url, payload, provider });
        return;
      }
      const method = memberName(callee);
      if (method === "mutateAsync") {
        const mutation = value(source, callee.object, env);
        if (mutation.kind === "mutation")
          trace(
            mutation.fn,
            call.arguments.map((arg) => value(source, arg, env)),
            requests,
            provider,
            conditions,
          );
        return;
      }
      const target = value(source, callee, env);
      if (target.kind === "function")
        trace(
          target,
          call.arguments.map((arg) => value(source, arg, env)),
          requests,
          provider,
          conditions,
        );
    }
    function expressions(
      source,
      expression,
      env,
      requests,
      provider,
      conditions,
    ) {
      if (!expression) return;
      graph.walk(source, expression, (node) => {
        if (node.type === "CallExpression") {
          request(source, node, env, requests, provider, conditions);
          return false;
        }
      });
    }
    function trace(target, args, requests, provider = null, conditions = []) {
      if (
        target.kind !== "function" ||
        ++traceCount > 1000 ||
        tracing.size > 20
      )
        return;
      const id = `${target.id}(${args.map(key).join(",")})`;
      if (tracing.has(id)) return;
      tracing.add(id);
      graph.add(target.unit, target.fn);
      const env = bind(target, args);
      function statements(list, states) {
        for (const statement of list) {
          const next = [];
          for (const state of states) {
            graph.tick();
            if (statement.type === "ReturnStatement") {
              expressions(
                target.unit,
                statement.argument,
                state.env,
                requests,
                provider,
                state.conditions,
              );
              continue;
            }
            if (statement.type === "ThrowStatement") continue;
            if (statement.type === "IfStatement") {
              for (const desired of [true, false]) {
                if (
                  !possible(
                    target.unit,
                    statement.test,
                    state.env,
                    desired,
                    state.conditions,
                  )
                )
                  continue;
                const branch = desired
                  ? statement.consequent
                  : statement.alternate;
                const conditions = [
                  ...state.conditions,
                  {
                    source: target.unit,
                    test: statement.test,
                    env: state.env,
                    desired,
                  },
                ];
                next.push(
                  ...(branch
                    ? statements(
                        branch.type === "BlockStatement"
                          ? branch.body
                          : [branch],
                        [{ env: state.env, conditions }],
                      )
                    : [{ env: state.env, conditions }]),
                );
              }
            } else if (statement.type === "TryStatement") {
              next.push(...statements(statement.block.body, [state]));
            } else if (statement.type === "BlockStatement")
              next.push(...statements(statement.body, [state]));
            else {
              if (statement.type === "VariableDeclaration")
                for (const item of statement.declarations)
                  expressions(
                    target.unit,
                    item.init,
                    state.env,
                    requests,
                    provider,
                    state.conditions,
                  );
              else if (statement.type === "ExpressionStatement")
                expressions(
                  target.unit,
                  statement.expression,
                  state.env,
                  requests,
                  provider,
                  state.conditions,
                );
              next.push(state);
            }
          }
          states = next;
          if (states.length > 32)
            throw new Error("HTTP branch budget exceeded");
        }
        return states;
      }
      if (target.fn.body.type === "BlockStatement")
        statements(target.fn.body.body, [{ env, conditions }]);
      else
        expressions(
          target.unit,
          target.fn.body,
          env,
          requests,
          provider,
          conditions,
        );
      tracing.delete(id);
    }
    function providerFor(source, expression, env = new Map()) {
      const result = value(source, expression, env);
      return result.kind === "object" && result.provider ? result : null;
    }
    function pathShape(url) {
      if (url.kind === "literal" && typeof url.value === "string")
        return { shape: url.value, keys: [] };
      if (url.kind !== "string") return null;
      const parts = [...url.parts];
      // An invariant base URL can precede the source-proven absolute API path.
      if (typeof parts[0] !== "string" || !parts[0]) {
        while (
          parts.length &&
          (typeof parts[0] !== "string" || !parts[0].startsWith("/"))
        ) {
          const part = parts.shift();
          if (typeof part !== "string" && (part?.iteration || part?.uncertain))
            return null;
        }
      }
      let shape = "",
        keys = [];
      for (const part of parts) {
        if (typeof part === "string") shape += part;
        else {
          if (part.iteration || part.uncertain || !shape.endsWith("/"))
            return null;
          shape += ":key";
          keys.push(key(part));
        }
      }
      return shape.startsWith("/") && !/[?#]/.test(shape)
        ? { shape, keys }
        : null;
    }
    function ownedBody(payload, provider) {
      if (payload.kind !== "object" || payload.opaque || !payload.iteration)
        return false;
      const identities = [...provider.fields.values()]
        .filter(
          (item) => item.kind === "atom" && !item.iteration && !item.uncertain,
        )
        .map(key);
      return [...payload.fields.values()].some(
        (item) =>
          !item.iteration && !item.uncertain && identities.includes(key(item)),
      );
    }
    function observedOutput(source, loop, node, provider) {
      let push = source.parents.get(node);
      while (
        push &&
        push !== loop &&
        !(push.type === "CallExpression" && memberName(push.callee) === "push")
      )
        push = source.parents.get(push);
      if (
        !push ||
        push === loop ||
        unwrap(push.callee.object)?.type !== "Identifier"
      )
        return false;
      const output = source.binding(unwrap(push.callee.object));
      if (
        !immutable(output) ||
        output.defs[0].node.init?.type !== "ArrayExpression" ||
        output.defs[0].node.init.elements.length
      )
        return false;
      const fn = graph.owner(source, loop),
        returns = descendants(
          source,
          fn.body,
          (item) =>
            item.type === "ReturnStatement" &&
            item.argument?.type === "ObjectExpression",
        );
      if (
        !returns.some((item) =>
          item.argument.properties.some(
            (field) =>
              field.type === "Property" &&
              source.binding(unwrap(field.value)) === output,
          ),
        )
      )
        return false;
      const call = source.parents.get(fn),
        commit =
          call?.type === "CallExpression" &&
          call.arguments.includes(fn) &&
          graph.fn(source, call.callee);
      if (!commit || commit.unit !== source) return false;
      const parameter = source.binding(
        unwrap(commit.fn.params[call.arguments.indexOf(fn)]),
      );
      const candidate = descendants(
        source,
        commit.fn.body,
        (item) =>
          item.type === "VariableDeclarator" &&
          item.id.type === "Identifier" &&
          unwrap(item.init)?.type === "AwaitExpression" &&
          unwrap(item.init.argument)?.type === "CallExpression" &&
          source.binding(unwrap(item.init.argument.callee)) === parameter,
      );
      if (candidate.length !== 1) return false;
      const binding = source.binding(candidate[0].id),
        calls = descendants(
          source,
          commit.fn.body,
          (item) =>
            item.type === "CallExpression" &&
            item.range[0] > candidate[0].range[1] &&
            item.arguments.some(
              (arg) => source.binding(unwrap(arg)) === binding,
            ),
        );
      for (const persist of calls) {
        const fn = value(source, persist.callee, new Map());
        if (fn.kind !== "function") continue;
        const arguments_ = persist.arguments.map((arg) =>
          source.binding(unwrap(arg)) === binding
            ? atom("observed-candidate", { candidate: true })
            : value(source, arg, new Map()),
        );
        if (!arguments_.some((arg) => key(arg) === key(provider))) continue;
        const events = [];
        trace(fn, arguments_, events, provider.provider);
        if (
          events.some(
            (event) =>
              event.payload.candidate &&
              pathShape(event.url) &&
              contracts.matching(event.method, pathShape(event.url).shape),
          )
        ) {
          graph.add(source, returns[0]);
          graph.add(source, commit.fn);
          graph.add(source, candidate[0]);
          return true;
        }
      }
      return false;
    }
    try {
      const source = graph.read(unit.file);
      const node = source.nodes.find(
        (item) =>
          item.type === "AwaitExpression" && item.range[0] === awaited.range[0],
      );
      if (!node) return null;
      let loop = source.parents.get(node);
      while (
        loop &&
        !FUNCTIONS.has(loop.type) &&
        loop.type !== "ForOfStatement"
      )
        loop = source.parents.get(loop);
      if (
        loop?.type !== "ForOfStatement" ||
        loop.await ||
        loop.left.type !== "VariableDeclaration" ||
        loop.left.declarations.length !== 1 ||
        loop.left.declarations[0].id.type !== "Identifier"
      )
        return null;
      const iteration = source.binding(loop.left.declarations[0].id),
        env = new Map([
          [iteration, atom("current-loop-item", { iteration: true })],
        ]);
      const call = unwrap(node.argument);
      if (
        call?.type !== "CallExpression" ||
        call.optional ||
        call.arguments.some((arg) => arg.type === "SpreadElement")
      )
        return null;
      const inputs = call.arguments.map((arg) => value(source, arg, env));
      const owners = call.arguments
        .map((arg) => providerFor(source, arg, env))
        .filter(Boolean);
      if (owners.length !== 1 || !inputs.some((input) => input.iteration))
        return null;
      const provider = owners[0],
        target = value(source, call.callee, env);
      if (target.kind !== "function") return null;
      const events = [];
      trace(target, inputs, events, provider.provider);
      const witness = events.find((event) => {
        const uri = pathShape(event.url);
        return (
          uri &&
          uri.keys.length > 0 &&
          ownedBody(event.payload, provider) &&
          contracts.matching(event.method, uri.shape)
        );
      });
      if (!witness || !observedOutput(source, loop, node, provider))
        return null;
      graph.add(source, node);
      graph.add(source, loop);
      if (
        graph
          .evidence()
          .some(
            (entry) =>
              graph.units.has(entry.file) &&
              !graph.nativeClosed(graph.units.get(entry.file)),
          )
      )
        return null;
      return {
        predicate: "declared-shared-http-mutation-order",
        reason:
          "The awaited item operation reaches a source-bound, officially declared mutating API through the same host and overlapping collection owner keys. Its ordered results are persisted by that owner. These declared shared mutations have no established independence, so the rule's unconditional parallelization premise is unsupported; this does not claim that every POST is noncommutative. The raw diagnostic is retained.",
        evidence: graph.evidence(),
      };
    } catch {
      return null;
    }
  };
}
module.exports = { createSharedHttpMutationProof };
