// Preserve offsets and newlines while removing text that cannot be Rust code.
function maskRustText(source) {
  const chars = source.split('');
  const mask = (start, end) => {
    for (let i = start; i < end; i += 1) if (chars[i] !== '\n' && chars[i] !== '\r') chars[i] = ' ';
  };
  for (let i = 0; i < source.length;) {
    const start = i;
    let quoted = false;
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
