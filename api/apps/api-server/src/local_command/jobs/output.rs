use super::super::process;
use super::{lock, JobError, OutputStream, ReadInput, ReadOutput};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

pub(super) struct Output {
    stdout: File,
    stderr: File,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub discarded_bytes: u64,
    limit: u64,
}
impl Output {
    pub fn new(limit: u64) -> io::Result<Self> {
        Ok(Self {
            stdout: process::private_output_file()?,
            stderr: process::private_output_file()?,
            stdout_bytes: 0,
            stderr_bytes: 0,
            discarded_bytes: 0,
            limit,
        })
    }
    pub fn read(&mut self, input: ReadInput, complete: bool) -> Result<ReadOutput, JobError> {
        if input.max_bytes == 0 {
            return Err(JobError::InvalidInput("max_bytes must be positive".into()));
        }
        let budget = u64::try_from(input.max_bytes)
            .map_err(|_| JobError::InvalidInput("max_bytes overflow".into()))?;
        input
            .offset
            .checked_add(budget)
            .ok_or_else(|| JobError::InvalidInput("read cursor overflow".into()))?;
        let (file, size) = match input.stream {
            OutputStream::Stdout => (&mut self.stdout, self.stdout_bytes),
            OutputStream::Stderr => (&mut self.stderr, self.stderr_bytes),
        };
        if input.offset > size {
            return Err(JobError::InvalidInput(
                "offset exceeds available output".into(),
            ));
        }
        let count = usize::try_from((size - input.offset).min(budget))
            .map_err(|_| JobError::InvalidInput("read size overflow".into()))?;
        // Allocate only the bytes that exist, and report allocation failures instead of panicking.
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(count).map_err(|_| {
            JobError::InvalidInput("read allocation exceeds available memory".into())
        })?;
        bytes.resize(count, 0);
        file.seek(SeekFrom::Start(input.offset))?;
        file.read_exact(&mut bytes)?;
        let at_end = input.offset + count as u64 == size;
        let mut cursor = 0;
        let mut consumed = bytes.len();
        while cursor < bytes.len() {
            match std::str::from_utf8(&bytes[cursor..]) {
                Ok(_) => break,
                Err(error) => {
                    cursor += error.valid_up_to();
                    if let Some(length) = error.error_len() {
                        cursor += length;
                    } else if !at_end || !complete {
                        consumed = cursor;
                        if consumed == 0 {
                            let width = match bytes[0] {
                                0xc2..=0xdf => 2,
                                0xe0..=0xef => 3,
                                0xf0..=0xf4 => 4,
                                _ => 1,
                            };
                            if input.max_bytes < width {
                                return Err(JobError::InvalidInput(format!(
                                    "max_bytes must fit the next UTF-8 character ({width} bytes)"
                                )));
                            }
                        }
                        break;
                    } else {
                        break;
                    }
                }
            }
        }
        let text = String::from_utf8_lossy(&bytes[..consumed]);
        let encoding_lossy = matches!(text, std::borrow::Cow::Owned(_));
        let next_offset = input.offset + consumed as u64;
        Ok(ReadOutput {
            text: text.into_owned(),
            next_offset,
            eof: complete && next_offset == size,
            encoding_lossy,
        })
    }
}

pub(super) struct Sink {
    pub output: Arc<Mutex<Output>>,
    pub stream: OutputStream,
}
impl process::RawOutput for Sink {
    fn append(&self, bytes: &[u8]) -> io::Result<()> {
        let mut output = lock(&self.output);
        let remaining = output.limit - output.stdout_bytes - output.stderr_bytes;
        let kept = usize::try_from(remaining.min(bytes.len() as u64))
            .map_err(|_| io::Error::other("output size overflow"))?;
        let file = match self.stream {
            OutputStream::Stdout => &mut output.stdout,
            OutputStream::Stderr => &mut output.stderr,
        };
        file.seek(SeekFrom::End(0))?;
        file.write_all(&bytes[..kept])?;
        match self.stream {
            OutputStream::Stdout => output.stdout_bytes += kept as u64,
            OutputStream::Stderr => output.stderr_bytes += kept as u64,
        }
        output.discarded_bytes = output
            .discarded_bytes
            .checked_add((bytes.len() - kept) as u64)
            .ok_or_else(|| io::Error::other("discarded output counter overflow"))?;
        // Continue draining both pipes after the shared disk budget is exhausted.
        Ok(())
    }
}
