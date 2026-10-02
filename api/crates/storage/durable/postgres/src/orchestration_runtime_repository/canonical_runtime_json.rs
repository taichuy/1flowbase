//! Canonical identity is inseparable from the immutable Value it was computed from.
use super::{Result, Value};
use sha2::{Digest, Sha256};

/// Private fields prevent sibling writers from supplying another Value or digest.
pub(super) struct PreparedCanonicalRuntimeJson<'a> {
    value: &'a Value,
    hash: String,
    byte_size: i64,
}

impl<'a> PreparedCanonicalRuntimeJson<'a> {
    pub(super) fn new(value: &'a Value) -> Result<Self> {
        let (hash, byte_size) = identity(value)?;
        Ok(Self {
            value,
            hash,
            byte_size,
        })
    }

    pub(super) fn value(&self) -> &'a Value {
        self.value
    }

    pub(super) fn hash(&self) -> &str {
        &self.hash
    }

    pub(super) fn byte_size(&self) -> i64 {
        self.byte_size
    }
}

fn write_canonical_runtime_json<W: std::io::Write>(
    value: &serde_json::Value,
    output: &mut W,
) -> serde_json::Result<()> {
    let write = |out: &mut W, bytes: &[u8]| out.write_all(bytes).map_err(serde_json::Error::io);
    match value {
        Value::Object(object) => {
            write(output, b"{")?;
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    write(output, b",")?;
                }
                serde_json::to_writer(&mut *output, key)?;
                write(output, b":")?;
                write_canonical_runtime_json(&object[key], output)?;
            }
            write(output, b"}")?;
        }
        Value::Array(items) => {
            write(output, b"[")?;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    write(output, b",")?;
                }
                write_canonical_runtime_json(item, output)?;
            }
            write(output, b"]")?;
        }
        scalar => serde_json::to_writer(output, scalar)?,
    }
    Ok(())
}

fn identity(value: &Value) -> Result<(String, i64)> {
    struct HashWriter {
        hash: Sha256,
        length: i64,
    }
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.length = self
                .length
                .checked_add(i64::try_from(bytes.len()).map_err(std::io::Error::other)?)
                .ok_or_else(|| std::io::Error::other("canonical JSON length overflow"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = HashWriter {
        hash: Sha256::new(),
        length: 0,
    };
    write_canonical_runtime_json(value, &mut output)?;
    Ok((
        format!("sha256:{:x}", output.hash.finalize()),
        output.length,
    ))
}

#[cfg(test)]
#[path = "_tests/canonical_runtime_json.rs"]
mod tests;
