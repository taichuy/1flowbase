//! A deliberately non-executable dotenv reader: no shell evaluation or interpolation.
use anyhow::{ensure, Context, Result};
use std::{collections::BTreeMap, fs, path::Path};

pub type Values = BTreeMap<String, String>;

pub fn parse(text: &str) -> Result<Values> {
    let mut values = Values::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, raw) = line
            .split_once('=')
            .with_context(|| format!("Invalid .env line {}", index + 1))?;
        let key = key.trim();
        ensure!(
            !key.is_empty()
                && key.bytes().enumerate().all(|(i, c)| c == b'_'
                    || c.is_ascii_alphabetic()
                    || (i > 0 && c.is_ascii_digit())),
            "Invalid .env key on line {}",
            index + 1
        );
        let raw = raw.trim();
        let value = if raw.starts_with('"') {
            let mut reader = serde_json::Deserializer::from_str(raw).into_iter::<String>();
            let value = reader.next().context("Missing quoted value")??;
            let tail = raw[reader.byte_offset()..].trim();
            ensure!(
                tail.is_empty() || tail.starts_with('#'),
                "Unexpected text after .env value on line {}",
                index + 1
            );
            value
        } else if let Some(raw) = raw.strip_prefix('\'') {
            let (value, tail) = raw
                .split_once('\'')
                .context("Unclosed single quote in .env")?;
            ensure!(
                tail.trim().is_empty() || tail.trim().starts_with('#'),
                "Unexpected text after .env value"
            );
            value.to_owned()
        } else {
            raw.split_once(" #")
                .map_or(raw, |(value, _)| value)
                .trim_end()
                .to_owned()
        };
        ensure!(
            !value.contains('\0'),
            "NUL bytes are not valid environment values"
        );
        ensure!(
            values.insert(key.to_owned(), value).is_none(),
            "Duplicate .env key: {key}"
        );
    }
    Ok(values)
}

pub fn read(root: &Path) -> Result<Values> {
    let path = root.join(".env");
    parse(&fs::read_to_string(&path).with_context(|| {
        format!(
            "Cannot read {}; use the .env included in the native package",
            path.display()
        )
    })?)
}

pub fn required<'a>(values: &'a Values, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .with_context(|| format!("Missing {key} in .env"))
}

pub fn persist_generated(root: &Path, generated: &Values) -> Result<()> {
    if generated.is_empty() {
        return Ok(());
    }
    let path = root.join(".env");
    let original = fs::read_to_string(&path)?;
    let mut remaining = generated.clone();
    let mut lines = Vec::new();
    for line in original.lines() {
        let key = line
            .trim()
            .strip_prefix("export ")
            .unwrap_or(line.trim())
            .split_once('=')
            .map(|(key, _)| key.trim());
        if let Some(value) = key.and_then(|key| remaining.remove(key).map(|v| (key, v))) {
            lines.push(format!("{}={}", value.0, serde_json::to_string(&value.1)?));
        } else {
            lines.push(line.to_owned());
        }
    }
    for (key, value) in remaining {
        lines.push(format!("{key}={}", serde_json::to_string(&value)?));
    }
    let temporary = root.join(format!(".env-{}.tmp", std::process::id()));
    let outcome = (|| -> Result<()> {
        use std::io::Write;
        let mut file = super::settings::private_file(&temporary)?;
        file.write_all(format!("{}\n", lines.join("\n")).as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome
}

#[cfg(test)]
#[path = "_tests/env_file.rs"]
mod tests;
