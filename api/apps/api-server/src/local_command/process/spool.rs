use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
};

pub(super) struct Spool {
    file: File,
    pub encoding_lossy: bool,
}
impl Spool {
    pub fn new() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!("1flowbase-command-{}", uuid::Uuid::now_v7()));
        let mut options = OpenOptions::new();
        options.create_new(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                DELETE, FILE_FLAG_DELETE_ON_CLOSE, FILE_SHARE_DELETE, FILE_SHARE_READ,
                FILE_SHARE_WRITE,
            };
            options
                .access_mode(
                    windows_sys::Win32::Foundation::GENERIC_READ
                        | windows_sys::Win32::Foundation::GENERIC_WRITE
                        | DELETE,
                )
                .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
                .share_mode(FILE_SHARE_DELETE | FILE_SHARE_READ | FILE_SHARE_WRITE);
        }
        let file = options.open(&path)?;
        #[cfg(unix)]
        std::fs::remove_file(&path)?;
        Ok(Self {
            file,
            encoding_lossy: false,
        })
    }
    pub fn into_file(mut self) -> io::Result<File> {
        self.file.seek(SeekFrom::Start(0))?;
        Ok(self.file)
    }
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        let text = String::from_utf8_lossy(bytes);
        self.encoding_lossy |= matches!(text, std::borrow::Cow::Owned(_));
        // At most one I/O chunk is decoded/escaped, never the complete result.
        let json = serde_json::to_vec(text.as_ref())?;
        self.file.write_all(&json[1..json.len() - 1])
    }
}

pub(super) fn capture(mut reader: impl Read, mut spool: Spool) -> io::Result<Spool> {
    let mut chunk = [0; 16 * 1024]; // I/O granularity, not an output quota.
    let mut pending = Vec::with_capacity(chunk.len() + 4);
    loop {
        let n = match reader.read(&mut chunk) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        if n == 0 {
            break;
        }
        pending.extend_from_slice(&chunk[..n]);
        let mut cursor = 0;
        let mut complete = pending.len();
        while cursor < pending.len() {
            match std::str::from_utf8(&pending[cursor..]) {
                Ok(_) => break,
                Err(error) => {
                    cursor += error.valid_up_to();
                    if let Some(length) = error.error_len() {
                        cursor += length;
                    } else {
                        complete = cursor;
                        break;
                    }
                }
            }
        }
        spool.append(&pending[..complete])?;
        pending.drain(..complete);
    }
    spool.append(&pending)?;
    spool.file.flush()?;
    Ok(spool)
}
