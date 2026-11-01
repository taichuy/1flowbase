use crate::{ErrorKind, ExecutionError};

pub(crate) fn reject_module_syntax(source: &str, legacy: bool) -> Result<(), ExecutionError> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                index = skip_quoted_literal(bytes, index, bytes[index]);
                continue;
            }
            b'`' => {
                index = if legacy {
                    skip_quoted_literal(bytes, index, b'`')
                } else {
                    skip_template(bytes, index)?
                };
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = skip_line_comment(bytes, index + 2);
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index + 2);
                continue;
            }
            _ => {}
        }
        if keyword_at(bytes, index, b"import") {
            let next = skip_ascii_whitespace(bytes, index + b"import".len());
            if next < bytes.len() && bytes[next] == b'(' {
                return Err(ExecutionError::from(ErrorKind::DynamicImportDenied).into());
            }
            return Err(ExecutionError::from(ErrorKind::ModuleImportDenied).into());
        }
        if keyword_at(bytes, index, b"export") {
            return Err(ExecutionError::from(ErrorKind::ModuleImportDenied).into());
        }
        index += 1;
    }

    Ok(())
}

fn keyword_at(source: &[u8], index: usize, keyword: &[u8]) -> bool {
    source
        .get(index..index + keyword.len())
        .is_some_and(|candidate| candidate == keyword)
        && index
            .checked_sub(1)
            .and_then(|previous| source.get(previous))
            .is_none_or(|byte| !is_js_identifier_byte(*byte))
        && source
            .get(index + keyword.len())
            .is_none_or(|byte| !is_js_identifier_byte(*byte))
}

fn skip_ascii_whitespace(source: &[u8], mut index: usize) -> usize {
    while source
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn skip_quoted_literal(source: &[u8], mut index: usize, quote: u8) -> usize {
    index += 1;
    while index < source.len() {
        if source[index] == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if source[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    source.len()
}

fn skip_line_comment(source: &[u8], mut index: usize) -> usize {
    while index < source.len() && source[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(source: &[u8], mut index: usize) -> usize {
    while index + 1 < source.len() {
        if source[index] == b'*' && source[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    source.len()
}

fn is_js_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

pub(crate) fn is_js_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_' || first == b'$')
        && bytes.all(is_js_identifier_byte)
}

fn skip_template(source: &[u8], mut index: usize) -> Result<usize, ExecutionError> {
    index += 1;
    while index < source.len() {
        match source[index] {
            b'\\' => index = index.saturating_add(2),
            b'`' => return Ok(index + 1),
            b'$' if source.get(index + 1) == Some(&b'{') => {
                // A conservative expression scan: recursively protect quoted/template text,
                // then reject import/export tokens. No module loader is installed either.
                index += 2;
                let mut depth = 1;
                while index < source.len() && depth > 0 {
                    match source[index] {
                        b'\'' | b'"' => {
                            index = skip_quoted_literal(source, index, source[index]);
                            continue;
                        }
                        b'`' => {
                            index = skip_template(source, index)?;
                            continue;
                        }
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    if keyword_at(source, index, b"import") {
                        return Err(ErrorKind::DynamicImportDenied.into());
                    }
                    if keyword_at(source, index, b"export") {
                        return Err(ErrorKind::ModuleImportDenied.into());
                    }
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    Ok(index)
}
