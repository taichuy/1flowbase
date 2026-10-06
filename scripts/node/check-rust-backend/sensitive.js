// Operate on masked code: literal/comment text cannot become a field or log argument.
const { delimitedEnd } = require('./source.js');

const SENSITIVE_FIELD = /^(?:password_hash|token_hash|encrypted_secret_json|secret_value|api_key_secret)$/u;
const SENSITIVE_LOG = /\b(?:password|token|secret|api_key)\b/iu;

function sensitiveOccurrences(code) {
  const findings = [];
  const logging = /\b(?:tracing::(?:trace|debug|info|warn|error)|println|eprintln)!\s*([([{])/gu;
  for (const match of code.matchAll(logging)) {
    const start = match.index + match[0].length - 1;
    const end = delimitedEnd(code, start);
    if (end === null) continue;
    const sensitive = SENSITIVE_LOG.exec(code.slice(start + 1, end - 1));
    if (sensitive) findings.push({ rule: 'no-sensitive-logging', offset: start + 1 + sensitive.index });
  }

  // An attribute belongs only to the following declaration, never a later struct.
  for (const match of code.matchAll(/#\s*\[\s*derive\s*\(/gu)) {
    const start = code.indexOf('[', match.index);
    const end = delimitedEnd(code, start);
    if (end === null || !/\bSerialize\b/u.test(code.slice(start, end))) continue;
    let cursor = end;
    while (cursor < code.length) {
      while (/\s/u.test(code[cursor] || '') && cursor < code.length) cursor += 1;
      const attribute = /^#\s*\[/u.exec(code.slice(cursor));
      if (!attribute) break;
      const attributeEnd = delimitedEnd(code, cursor + attribute[0].length - 1);
      if (attributeEnd === null) break;
      cursor = attributeEnd;
    }
    const declaration = /^(?:pub(?:\([^)]*\))?\s+)?struct\s+\w+\s*/u.exec(code.slice(cursor));
    if (!declaration) continue;
    cursor += declaration[0].length;
    // Skip generic bounds, including const-generic expressions, before the body.
    if ('(;'.includes(code[cursor])) continue; // Unit and tuple structs.
    let angles = 0;
    for (; cursor < code.length; cursor += 1) {
      const char = code[cursor];
      if (char === '<') angles += 1;
      else if (char === '>' && code[cursor - 1] !== '-' && angles > 0) angles -= 1;
      else if ('(['.includes(char) || (angles > 0 && char === '{')) {
        const nestedEnd = delimitedEnd(code, cursor);
        if (nestedEnd === null) break;
        cursor = nestedEnd - 1;
      } else if (angles === 0 && '{;'.includes(char)) break;
    }
    if (code[cursor] !== '{') continue; // Unit/tuple structs have no named fields.
    const bodyEnd = delimitedEnd(code, cursor);
    if (bodyEnd === null) continue;
    for (const field of namedFields(code, cursor + 1, bodyEnd - 1)) {
      if (!SENSITIVE_FIELD.test(field.name)) continue;
      const skipped = field.attributes.some(attribute =>
        /#\s*\[\s*serde\s*\([^)]*\b(?:skip|skip_serializing)\b(?!\s*=)[^)]*\)\s*\]/u.test(attribute));
      if (!skipped) findings.push({ rule: 'no-sensitive-serialize', offset: field.offset });
    }
  }
  return findings;
}

// Visit only top-level named field declarations. Types may contain generic commas
// and const blocks with typed local bindings that are not serialized fields.
function* namedFields(code, start, end) {
  let cursor = start;
  while (cursor < end) {
    while (cursor < end && /[\s,]/u.test(code[cursor])) cursor += 1;
    const attributes = [];
    while (cursor < end) {
      const attribute = /^#\s*\[/u.exec(code.slice(cursor));
      if (!attribute) break;
      const attributeEnd = delimitedEnd(code, cursor + attribute[0].length - 1);
      if (attributeEnd === null) return;
      attributes.push(code.slice(cursor, attributeEnd));
      cursor = attributeEnd;
      while (cursor < end && /\s/u.test(code[cursor])) cursor += 1;
    }
    const declaration = /^(?:pub\s*(?:\([^)]*\)\s*)?)?((?:r#)?[A-Za-z_][A-Za-z_0-9]*)\s*:/u.exec(code.slice(cursor));
    if (!declaration) return;
    const name = declaration[1].replace(/^r#/u, '');
    const offset = cursor + declaration[0].indexOf(declaration[1]);
    yield { name, offset, attributes };
    cursor += declaration[0].length;
    let angles = 0;
    for (; cursor < end; cursor += 1) {
      const char = code[cursor];
      if ('([{'.includes(char)) {
        const nestedEnd = delimitedEnd(code, cursor);
        if (nestedEnd === null) return;
        cursor = nestedEnd - 1;
      } else if (char === '<') angles += 1;
      else if (char === '>' && code[cursor - 1] !== '-' && angles > 0) angles -= 1;
      else if (char === ',' && angles === 0) { cursor += 1; break; }
    }
  }
}

module.exports = { sensitiveOccurrences };
