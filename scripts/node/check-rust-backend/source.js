// Preserve offsets and newlines while removing text that cannot be Rust code.
function maskRustText(source) {
  const chars = source.split('');
  const mask = (start, end) => {
    for (let i = start; i < end; i += 1) if (chars[i] !== '\n' && chars[i] !== '\r') chars[i] = ' ';
  };
  for (let i = 0; i < source.length;) {
    const start = i;
    let quoted = false;
    let rawQuoted = false;
    if (source.startsWith('//', i)) {
      const end = source.indexOf('\n', i);
      i = end < 0 ? source.length : end;
    } else if (source.startsWith('/*', i)) {
      let depth = 1;
      i += 2;
      while (i < source.length && depth > 0) {
        if (source.startsWith('/*', i)) { depth += 1; i += 2; }
        else if (source.startsWith('*/', i)) { depth -= 1; i += 2; }
        else i += 1;
      }
    } else {
      const raw = source.slice(i).match(/^(?:br|cr|r)(#*)"/u);
      if (raw) {
        rawQuoted = true;
        const end = source.indexOf(`"${raw[1]}`, i + raw[0].length);
        i = end < 0 ? source.length : end + raw[1].length + 1;
      } else if (source[i] === '"') {
        quoted = true;
        i += 1;
        while (i < source.length) {
          if (source[i] === '\\') i += 2;
          else if (source[i++] === '"') break;
        }
      } else if (source[i] === "'") {
        // A lifetime such as 'static is code; only a closed character literal is masked.
        const character = source.slice(i).match(/^'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|[^\r\n])|[^'\\\r\n])'/u);
        if (!character) { i += 1; continue; }
        i += character[0].length;
      } else { i += 1; continue; }
    }
    mask(start, i);
    if (rawQuoted && i - start >= 2) {
      chars[start] = '"';
      chars[i - 1] = '"';
    }
    if (quoted) {
      chars[start] = '"';
      if (source[i - 1] === '"') chars[i - 1] = '"';
    }
  }
  return chars.join('');
}

// Three-valued evaluation with test=false: only a definitely false condition
// proves that an item cannot be compiled into any production configuration.
function isTestOnlyCfg(attribute) {
  const match = attribute.match(/^#\s*\[\s*cfg\s*\(([\s\S]*)\)\s*\]$/u);
  if (!match) return false;
  const tokens = match[1].match(/[A-Za-z_][A-Za-z_0-9]*|[(),=]|"(?:[^"\\]|\\.)*"/gu) || [];
  let cursor = 0;
  const parse = () => {
    const name = tokens[cursor++];
    if (!name || !/^[A-Za-z_]/u.test(name)) throw new Error('invalid cfg');
    if (tokens[cursor] === '(') {
      cursor += 1;
      const values = [];
      while (tokens[cursor] !== ')') {
        values.push(parse());
        if (tokens[cursor] !== ',') break;
        cursor += 1;
      }
      if (tokens[cursor++] !== ')') throw new Error('unclosed cfg');
      if (name === 'all') return values.includes(false) ? false : values.every(v => v === true) ? true : null;
      if (name === 'any') return values.includes(true) ? true : values.every(v => v === false) ? false : null;
      if (name === 'not' && values.length === 1) return values[0] === null ? null : !values[0];
      return null;
    }
    if (tokens[cursor] === '=') {
      cursor += 1;
      if (!tokens[cursor++]?.startsWith('"')) throw new Error('invalid cfg value');
      return null;
    }
    return name === 'test' ? false : null;
  };
  try { return parse() === false && cursor === tokens.length; }
  catch { return false; }
}

module.exports = { maskRustText, isTestOnlyCfg };

// Mask exactly the attributed item, preserving code after its closing delimiter.
// Strings/comments have already been masked, so delimiters here are syntax.
function maskTestOnlyItems(code) {
  const chars = code.split('');
  const attributes = /#\s*\[\s*cfg\s*\(([^\]]*)\)\s*\]/gu;
  for (const match of code.matchAll(attributes)) {
    if (!isTestOnlyCfg(match[0])) continue;
    let cursor = match.index + match[0].length;
    // Other attributes still belong to the same item.
    while (true) {
      while (/\s/u.test(code[cursor] || '') && cursor < code.length) cursor += 1;
      if (code.slice(cursor, cursor + 2) !== '#[') break;
      let brackets = 0;
      do {
        if (code[cursor] === '[') brackets += 1;
        if (code[cursor] === ']') brackets -= 1;
        cursor += 1;
      } while (cursor < code.length && (brackets > 0 || code[cursor - 1] === '#'));
    }
    const declaration = code.slice(cursor).replace(/^(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|unsafe|default|const)\s+)*(?:extern\s+"[^"]*"\s+)?/u, '');
    const blockItem = /^(?:fn|mod|impl|struct|enum|trait|union|macro_rules)\b/u.test(declaration);
    // Fields and enum variants end at commas; declarations such as const/use end at semicolons.
    const commaItem = !blockItem && !/^(?:const|static|use|type|let)\b/u.test(code.slice(cursor));
    let parentheses = 0;
    let brackets = 0;
    let braces = 0;
    let body = false;
    let angles = 0;
    let end = null;
    for (; cursor < code.length; cursor += 1) {
      const char = code[cursor];
      if (blockItem && !body && char === '<') angles += 1;
      else if (blockItem && !body && char === '>' && code[cursor - 1] !== '-' && angles > 0) angles -= 1;
      else if (char === '(') parentheses += 1;
      else if (char === ')') parentheses -= 1;
      else if (char === '[') brackets += 1;
      else if (char === ']') brackets -= 1;
      else if (char === '{') { braces += 1; if (angles === 0 && parentheses === 0 && brackets === 0) body = true; }
      else if (char === '}') {
        braces -= 1;
        if (blockItem && body && braces === 0 && parentheses === 0 && brackets === 0 && angles === 0) { end = cursor + 1; break; }
        if (braces < 0) break;
      } else if ((char === ';' || (commaItem && char === ',')) && braces === 0 && parentheses === 0 && brackets === 0) {
        end = cursor + 1; break;
      }
    }
    // Unrecognized/incomplete syntax stays visible rather than hiding production code.
    if (end === null) continue;
    for (let i = match.index; i < end; i += 1) if (chars[i] !== '\n' && chars[i] !== '\r') chars[i] = ' ';
  }
  return chars.join('');
}

module.exports.maskTestOnlyItems = maskTestOnlyItems;
