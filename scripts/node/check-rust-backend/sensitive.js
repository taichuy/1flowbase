// Operate on masked code: literal/comment text cannot become a field or log argument.
const { delimitedEnd } = require('./source.js');

const SENSITIVE_FIELD = /\b(?:password_hash|token_hash|encrypted_secret_json|secret_value|api_key_secret)\s*:/gu;
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
      if (code.slice(cursor, cursor + 2) !== '#[') break;
      const attributeEnd = delimitedEnd(code, cursor + 1);
      if (attributeEnd === null) break;
      cursor = attributeEnd;
    }
    const declaration = /^(?:pub(?:\([^)]*\))?\s+)?struct\s+\w+\s*/u.exec(code.slice(cursor));
    if (!declaration) continue;
    cursor += declaration[0].length;
    // Skip generic bounds, including const-generic expressions, before the body.
    let angles = 0;
    for (; cursor < code.length; cursor += 1) {
      const char = code[cursor];
      if (char === '<') angles += 1;
      else if (char === '>' && code[cursor - 1] !== '-' && angles > 0) angles -= 1;
      else if (angles > 0 && '([{'.includes(char)) {
        const nestedEnd = delimitedEnd(code, cursor);
        if (nestedEnd === null) break;
        cursor = nestedEnd - 1;
      } else if (angles === 0 && '{(;'.includes(char)) break;
    }
    if (code[cursor] !== '{') continue; // Unit/tuple structs have no named fields.
    const bodyEnd = delimitedEnd(code, cursor);
    if (bodyEnd === null) continue;
    const body = code.slice(cursor + 1, bodyEnd - 1);
    for (const field of body.matchAll(SENSITIVE_FIELD)) {
      // Attributes on this field end at its name; generic type commas are irrelevant.
      const attributes = /(?:#\s*\[[^\]]*\]\s*)+(?:pub(?:\([^)]*\))?\s+)?$/u.exec(body.slice(0, field.index));
      const skipped = attributes && /#\s*\[\s*serde\s*\([^)]*\b(?:skip|skip_serializing)\b(?!\s*=)[^)]*\)\s*\]/u.test(attributes[0]);
      if (!skipped) findings.push({ rule: 'no-sensitive-serialize', offset: cursor + 1 + field.index });
    }
  }
  return findings;
}

module.exports = { sensitiveOccurrences };
