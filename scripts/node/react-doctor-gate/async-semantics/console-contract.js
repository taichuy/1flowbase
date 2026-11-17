const fs = require("node:fs");
const path = require("node:path");
const {
  maskRustText,
  maskTestOnlyItems,
  delimitedEnd,
} = require("../../check-rust-backend/source.js");

function createConsoleContracts(graph) {
  const cache = new Map();
  let rustFiles, manifests, policies;
  function source(file) {
    if (!cache.has(file)) {
      const text = graph.text(file);
      cache.set(file, {
        file,
        text,
        code: maskTestOnlyItems(maskRustText(text)),
      });
    }
    return cache.get(file);
  }
  function string(value) {
    value = value.trim();
    if (/^"(?:[^"\\]|\\.)*"$/s.test(value)) {
      try {
        return JSON.parse(value);
      } catch {
        return null;
      }
    }
    const raw = /^r(#{0,8})"([\s\S]*)"\1$/.exec(value);
    return raw ? raw[2] : null;
  }
  function split(text, code, start = 0, end = code.length) {
    const result = [];
    let from = start;
    for (let index = start; index < end; index++) {
      graph.tick();
      if ("([{".includes(code[index])) {
        const next = delimitedEnd(code, index);
        if (next === null || next > end) return null;
        index = next - 1;
      } else if (code[index] === ",") {
        if (text.slice(from, index).trim())
          result.push({
            start: from,
            end: index,
            text: text.slice(from, index).trim(),
          });
        from = index + 1;
      }
    }
    if (text.slice(from, end).trim())
      result.push({ start: from, end, text: text.slice(from, end).trim() });
    return result;
  }
  function top(sourceUnit, expression) {
    const found = [];
    for (const match of sourceUnit.code.matchAll(expression)) {
      let depth = 0;
      for (let index = 0; index < match.index; index++) {
        if ("([{".includes(sourceUnit.code[index])) depth++;
        else if (")]}".includes(sourceUnit.code[index])) depth--;
      }
      if (depth === 0) found.push(match);
    }
    return found;
  }
  function crateRoot(file) {
    let folder = path.posix.dirname(file);
    while (folder !== ".") {
      if (fs.existsSync(path.join(graph.repoRoot, folder, "Cargo.toml")))
        return folder;
      folder = path.posix.dirname(folder);
    }
    return null;
  }
  function crateFile(name) {
    manifests ??= graph
      .files("api/crates", /^Cargo\.toml$/)
      .map((file) => ({ file, text: graph.text(file) }));
    const matches = manifests.filter((item) => {
      const pkg = /\[package\]([\s\S]*?)(?=\n\[|$)/.exec(item.text);
      return (
        pkg &&
        /^\s*name\s*=\s*"([^"]+)"/m.exec(pkg[1])?.[1].replaceAll("-", "_") ===
          name
      );
    });
    if (matches.length === 1) {
      graph.addText(matches[0].file);
      return path.posix.join(path.posix.dirname(matches[0].file), "src/lib.rs");
    }
    return null;
  }
  function moduleTarget(
    file,
    name,
    match,
    directory = path.posix.dirname(file),
  ) {
    const unit = source(file),
      after = match.index + match[0].length;
    const delimiter = unit.code.slice(after).match(/^\s*([;{])/);
    if (!delimiter) return null;
    if (delimiter[1] === "{") {
      const start = after + delimiter.index + delimiter[0].lastIndexOf("{");
      const end = delimitedEnd(unit.code, start);
      return end === null
        ? null
        : {
            file,
            start: start + 1,
            end: end - 1,
            moduleDir: path.posix.join(directory, name),
          };
    }
    const preceding = unit.text.slice(0, match.index);
    const attribute =
      /#\s*\[\s*path\s*=\s*("(?:[^"\\]|\\.)*")\s*\]\s*(?:pub\s*)?$/.exec(
        preceding,
      );
    const base = directory;
    const options = attribute
      ? [path.posix.normalize(path.posix.join(base, string(attribute[1])))]
      : [
          path.posix.join(base, name + ".rs"),
          path.posix.join(base, name, "mod.rs"),
        ];
    const candidates = options.filter((candidate) =>
      fs.existsSync(path.join(graph.repoRoot, candidate)),
    );
    return candidates.length === 1
      ? {
          file: candidates[0],
          start: 0,
          end: source(candidates[0]).code.length,
          moduleDir: path.posix.dirname(candidates[0]),
        }
      : null;
  }
  function resolve(file, parts, seen = new Set(), section = null) {
    graph.tick();
    const key = `${file}:${section?.start ?? 0}:${parts.join("::")}`;
    if (seen.has(key) || seen.size > 24 || !parts.length) return null;
    seen = new Set([...seen, key]);
    const original = source(file),
      start = section?.start ?? 0,
      end = section?.end ?? original.code.length;
    const unit = {
      ...original,
      code: original.code.slice(start, end),
      text: original.text.slice(start, end),
    };
    const name = parts[0];
    if (parts.length === 1) {
      const declarations = top(
        unit,
        new RegExp(`\\bpub\\s+(?:struct|enum|fn)\\s+${name}\\b`, "g"),
      );
      if (declarations.length === 1) return { file, name };
    }
    const modules = top(
      unit,
      new RegExp(`\\b(?:pub\\s+)?mod\\s+${name}\\b`, "g"),
    );
    if (modules.length === 1) {
      const match = {
        ...modules[0],
        index: modules[0].index + start,
        0: modules[0][0],
      };
      const target = moduleTarget(file, name, match, section?.moduleDir);
      if (target) return resolve(target.file, parts.slice(1), seen, target);
    }
    const results = [];
    for (const statement of top(unit, /\bpub\s+use\s+[^;]+;/g)) {
      const target = /^pub\s+use\s+([A-Za-z_][\w:]*)::\*\s*;/.exec(
        statement[0],
      );
      if (!target) continue;
      const segments = target[1].split("::");
      const first = segments.shift();
      const local = top(
        unit,
        new RegExp(`\\b(?:pub\\s+)?mod\\s+${first}\\b`, "g"),
      );
      let result = null;
      if (local.length === 1) {
        const targetModule = moduleTarget(
          file,
          first,
          { ...local[0], index: local[0].index + start, 0: local[0][0] },
          section?.moduleDir,
        );
        if (targetModule)
          result = resolve(
            targetModule.file,
            [...segments, ...parts],
            seen,
            targetModule,
          );
      } else {
        const root = crateFile(first);
        if (root) result = resolve(root, [...segments, ...parts], seen);
      }
      if (result) {
        graph.addText(file);
        results.push(result);
      }
    }
    const distinct = new Map(
      results.map((item) => [item.file + item.name, item]),
    );
    return distinct.size === 1 ? [...distinct.values()][0] : null;
  }
  function declarationOrigin(unit, type) {
    if (
      new RegExp(`\\b(?:struct|enum|type|trait|mod)\\s+${type}\\b`).test(
        unit.code,
      )
    )
      return null;
    const imports = [];
    for (const statement of top(unit, /\buse\s+[^;]+;/g)) {
      const group = /^use\s+([A-Za-z_][\w:]*)::\{([\s\S]*)\}\s*;/.exec(
        statement[0],
      );
      if (!group) continue;
      for (const item of group[2].split(",").map((value) => value.trim())) {
        const names = /^([A-Za-z_]\w*)(?:\s+as\s+([A-Za-z_]\w*))?$/.exec(item);
        if (!names || (names[2] ?? names[1]) !== type) continue;
        const root = crateRoot(unit.file);
        if (!root || !group[1].startsWith("crate::")) return null;
        const result = resolve(`${root}/src/lib.rs`, [
          ...group[1].slice(7).split("::"),
          names[1],
        ]);
        if (result) imports.push(result);
      }
    }
    return imports.length === 1 ? imports[0] : null;
  }
  function authoritative(origin) {
    const unit = source(origin.file),
      code = unit.code;
    if (
      origin.name !== "ConsoleInterfaceDeclaration" ||
      !/\bpub\s+mutating\s*:\s*bool\b/.test(code) ||
      !/if\s+\w+\.mutating\s*\{\s*InterfaceAuditPolicy::Mutating\s*\}\s*else\s*\{\s*InterfaceAuditPolicy::ReadOnly\s*\}/.test(
        code,
      ) ||
      !/RouteIdentity::new\s*\(\s*\w+\.method\s*,\s*\w+\.path\s*\)/.test(
        code,
      ) ||
      !/BindingId::new\s*\(\s*\w+\.binding_id\s*\)/.test(code) ||
      !/\bpub\s+fn\s+compile_registry\b/.test(code)
    )
      return false;
    if (/\b(?:struct|enum|type)\s+InterfaceAuditPolicy\b/.test(code))
      return false;
    rustFiles ??= graph.files("api/crates", /\.rs$/);
    policies ??= rustFiles
      .filter((file) => graph.matchesText(file, ["InterfaceAuditPolicy"]))
      .filter((file) =>
        /\bpub\s+enum\s+InterfaceAuditPolicy\b/.test(source(file).code),
      );
    if (
      policies.length !== 1 ||
      !/enum\s+InterfaceAuditPolicy\s*\{\s*ReadOnly\s*,\s*Mutating\s*,?\s*\}/.test(
        source(policies[0]).code,
      )
    )
      return false;
    graph.addText(origin.file);
    graph.addText(policies[0]);
    return true;
  }
  function declarations(file) {
    const unit = source(file),
      families = [];
    for (const match of top(
      unit,
      /\bconst\s+([A-Za-z_]\w*)\s*:\s*&\s*\[\s*([A-Za-z_]\w*)\s*\]\s*=\s*&\s*\[/g,
    )) {
      const origin = declarationOrigin(unit, match[2]);
      if (!origin || !authoritative(origin)) continue;
      const start = match.index + match[0].length - 1,
        end = delimitedEnd(unit.code, start);
      if (!end) continue;
      const entries = split(unit.text, unit.code, start + 1, end - 1),
        records = [];
      if (!entries) continue;
      let valid = true;
      for (const entry of entries) {
        const open = unit.code.indexOf("{", entry.start),
          close = open >= 0 && delimitedEnd(unit.code, open);
        if (
          !close ||
          unit.code.slice(entry.start, open).trim() !== match[2] ||
          unit.code.slice(close, entry.end).trim()
        ) {
          valid = false;
          break;
        }
        const fields = split(unit.text, unit.code, open + 1, close - 1),
          record = {
            start: unit.code.indexOf(match[2], entry.start),
            end: close,
          };
        const keys = new Set();
        for (const field of fields ?? []) {
          const value = /^\s*([a-z_]+)\s*:\s*([\s\S]+)$/.exec(field.text);
          if (
            !value ||
            keys.has(value[1]) ||
            ![
              "interface_id",
              "binding_id",
              "method",
              "path",
              "mutating",
            ].includes(value[1])
          ) {
            valid = false;
            break;
          }
          keys.add(value[1]);
          record[value[1]] =
            value[1] === "mutating"
              ? value[2].trim() === "true"
                ? true
                : value[2].trim() === "false"
                  ? false
                  : null
              : string(value[2]);
          if (record[value[1]] === null) valid = false;
        }
        if (keys.size !== 5 || !valid) {
          valid = false;
          break;
        }
        records.push(record);
      }
      const bindings = new Set(),
        routes = new Set(),
        classifications = new Map();
      for (const record of records) {
        const route = `${record.method} ${record.path}`;
        if (
          bindings.has(record.binding_id) ||
          routes.has(route) ||
          (classifications.has(record.interface_id) &&
            classifications.get(record.interface_id) !== record.mutating)
        )
          valid = false;
        bindings.add(record.binding_id);
        routes.add(route);
        classifications.set(record.interface_id, record.mutating);
      }
      const calls = [
        ...unit.code.matchAll(/\b([A-Za-z_]\w*)::compile_registry\s*\(/g),
      ].filter((call) => {
        const finish = delimitedEnd(unit.code, call.index + call[0].length - 1);
        const args =
          finish &&
          split(unit.text, unit.code, call.index + call[0].length, finish - 1);
        return (
          args?.length === 4 &&
          args[2].text === match[1] &&
          call[1] === "console_interface"
        );
      });
      if (valid && calls.length === 1) {
        graph.addText(file);
        families.push({ file, records, origin });
      }
    }
    return families;
  }
  function route(family, record) {
    const file = family.file.endsWith("/interface.rs")
      ? family.file.slice(0, -"/interface.rs".length) + ".rs"
      : null;
    if (!file || !fs.existsSync(path.join(graph.repoRoot, file))) return false;
    const unit = source(file);
    const functions = [
      ...unit.code.matchAll(/\bpub\s+async\s+fn\s+([A-Za-z_]\w*)\b/g),
    ];
    for (const fn of functions) {
      const body = unit.code.indexOf("{", fn.index),
        end = body >= 0 && delimitedEnd(unit.code, body);
      if (!end) continue;
      const text = unit.text.slice(body, end),
        code = unit.code.slice(body, end);
      const invocations = [...code.matchAll(/\b([A-Za-z_]\w*)\s*\(/g)].filter(
        (call) => {
          const finish = delimitedEnd(code, call.index + call[0].length - 1);
          const args =
            finish &&
            split(text, code, call.index + call[0].length, finish - 1);
          return (
            args?.length === 5 &&
            string(args[2].text) === record.binding_id &&
            args[4].text === "true"
          );
        },
      );
      if (invocations.length !== 1) continue;
      const invokeName = invocations[0][1];
      const helper = new RegExp(`\\basync\\s+fn\\s+${invokeName}\\b`).exec(
        unit.code,
      );
      if (!helper) continue;
      const helperOpen = unit.code.indexOf("{", helper.index),
        helperEnd = delimitedEnd(unit.code, helperOpen);
      const helperCode = unit.code.slice(helperOpen, helperEnd);
      if (
        !/if\s+mutating\s*\{[\s\S]*::ProtocolWithCsrf\b[\s\S]*\}\s*else\s*\{[\s\S]*::Protocol\b/.test(
          helperCode,
        ) ||
        !/console_interface::invoke\s*\(/.test(helperCode)
      )
        continue;
      // Select the adjacent attribute, rather than a regex spanning earlier
      // handlers and accidentally reading their HTTP method.
      const before = unit.code.slice(0, fn.index),
        attribute = [...before.matchAll(/#\s*\[\s*utoipa::path\s*\(/g)].at(-1),
        attributeOpen = attribute && attribute.index + attribute[0].length - 1,
        attributeEnd = attribute && delimitedEnd(unit.code, attributeOpen),
        attr =
          attributeEnd &&
          /^\s*\]\s*$/.test(unit.code.slice(attributeEnd, fn.index))
            ? unit.text.slice(attributeOpen + 1, attributeEnd - 1)
            : null;
      if (
        !attr ||
        !new RegExp(`^\\s*${record.method.toLowerCase()}\\s*,`).test(attr) ||
        (!attr.includes(
          `path = ${JSON.stringify(record.path.replaceAll(/:([A-Za-z_]\w*)/g, "{$1}"))}`,
        ) &&
          !attr.includes(`path = ${JSON.stringify(record.path)}`))
      )
        continue;
      let registered = false;
      for (const match of unit.code.matchAll(/\.route\s*\(/g)) {
        const finish = delimitedEnd(
          unit.code,
          match.index + match[0].length - 1,
        );
        const args =
          finish &&
          split(
            unit.text,
            unit.code,
            match.index + match[0].length,
            finish - 1,
          );
        const routePath = args && string(args[0]?.text ?? "");
        if (
          !routePath ||
          !record.path.endsWith(routePath) ||
          !new RegExp(`\\b${fn[1]}\\b`).test(args[1]?.text ?? "")
        )
          continue;
        if (
          new RegExp(
            `(?:console_|\\.)${record.method.toLowerCase()}\\s*\\(`,
          ).test(args[1].text)
        )
          registered = true;
      }
      if (!registered) continue;
      graph.addText(file);
      return true;
    }
    return false;
  }
  let families;
  function matching(method, shape) {
    if (!["POST", "PATCH"].includes(method)) return null;
    families ??= graph
      .files("api/crates", /\.rs$/)
      // The canonical type spelling is required by the resolved import, even
      // when the local declaration type is aliased. Keep every lexical match;
      // comments/strings and false origins are rejected by full masked parsing.
      .filter((file) =>
        graph.matchesText(file, ["ConsoleInterfaceDeclaration"]),
      )
      .flatMap(declarations);
    const matches = [];
    for (const family of families)
      for (const record of family.records) {
        const normalized = record.path.replace(/:[A-Za-z_]\w*/g, ":key");
        if (
          record.method === method &&
          record.mutating &&
          normalized === shape &&
          route(family, record)
        ) {
          graph.addText(family.file, record.start, record.end);
          matches.push({ family, record });
        }
      }
    return matches.length === 1 ? matches[0] : null;
  }
  return { matching };
}
module.exports = { createConsoleContracts };
