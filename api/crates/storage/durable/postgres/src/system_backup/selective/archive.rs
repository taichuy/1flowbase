use anyhow::{ensure, Context, Result};
use control_plane_contracts::{
    ports::{BackupComponentReader, BackupComponentWriter},
    system_backup::selective::SelectiveBackupSelection,
};
use flate2::{
    write::{GzEncoder, MultiGzDecoder},
    Compression,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Write, path::PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) const FORMAT: &str = "1flowbase.settings-rows/v1";
pub(super) const MAX_LINE: u64 = 64 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Column {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub generated: bool,
    pub generated_expression: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Table {
    pub name: String,
    pub feature_id: String,
    pub data: bool,
    pub prerequisite: bool,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub plugin_owners: Vec<String>,
    pub prerequisites: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaDependency {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub format: String,
    pub selection: Vec<SelectiveBackupSelection>,
    pub tables: Vec<Table>,
    pub plugins: Vec<String>,
    pub schema_dependencies: Vec<SchemaDependency>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub table: String,
    pub row: Value,
}

/// The scratch file is private and removed on every exit, including transaction failure.
pub(super) struct Scratch {
    path: PathBuf,
}
impl Scratch {
    pub fn new() -> Result<(Self, tokio::fs::File)> {
        let path =
            std::env::temp_dir().join(format!("1flowbase-settings-{}.jsonl", uuid::Uuid::now_v7()));
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        Ok((Self { path }, tokio::fs::File::from_std(file)))
    }
    pub async fn reader(&self) -> Result<BufReader<tokio::fs::File>> {
        Ok(BufReader::new(tokio::fs::File::open(&self.path).await?))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
pub(super) async fn unpack(mut reader: BackupComponentReader) -> Result<Scratch> {
    let (scratch, mut file) = Scratch::new()?;
    let mut decoder = MultiGzDecoder::new(Vec::new());
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        decoder
            .write_all(&buffer[..count])
            .context("invalid compressed settings backup")?;
        let decoded = std::mem::take(decoder.get_mut());
        file.write_all(&decoded).await?;
    }
    let decoded = decoder
        .finish()
        .context("truncated compressed settings backup")?;
    file.write_all(&decoded).await?;
    file.flush().await?;
    Ok(scratch)
}
pub(super) async fn line(reader: &mut BufReader<tokio::fs::File>) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let count = reader
        .take(MAX_LINE + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    if count == 0 {
        return Ok(None);
    }
    ensure!(
        bytes.len() as u64 <= MAX_LINE && bytes.last() == Some(&b'\n'),
        "settings backup row exceeds limit or is truncated"
    );
    Ok(Some(bytes))
}
pub(super) async fn header(reader: &mut BufReader<tokio::fs::File>) -> Result<Header> {
    let bytes = line(reader).await?.context("empty settings backup")?;
    let header: Header = serde_json::from_slice(&bytes)?;
    ensure!(header.format == FORMAT, "unsupported settings rows format");
    ensure!(
        header.tables.len() + header.schema_dependencies.len() <= 10000,
        "too many settings tables"
    );
    Ok(header)
}
pub(super) struct Encoder {
    gzip: GzEncoder<Vec<u8>>,
    writer: BackupComponentWriter,
}
impl Encoder {
    pub fn new(writer: BackupComponentWriter) -> Self {
        Self {
            gzip: GzEncoder::new(Vec::new(), Compression::default()),
            writer,
        }
    }
    pub async fn write<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let mut row = serde_json::to_vec(value)?;
        row.push(b'\n');
        ensure!(
            row.len() as u64 <= MAX_LINE,
            "settings backup row exceeds limit"
        );
        self.gzip.write_all(&row)?;
        if self.gzip.get_ref().len() >= 64 * 1024 {
            self.writer
                .write_all(&std::mem::take(self.gzip.get_mut()))
                .await?;
        }
        Ok(())
    }
    pub async fn finish(mut self) -> Result<()> {
        self.gzip.try_finish()?;
        self.writer.write_all(self.gzip.get_ref()).await?;
        self.writer.flush().await?;
        Ok(())
    }
}
