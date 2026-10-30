use crate::local_command::process::{CompletedCommand, ResponsePart};
use axum::{body::Body, http::header::CONTENT_TYPE, response::Response};
use std::{collections::VecDeque, io};
use tokio::io::AsyncReadExt;

enum Part {
    Bytes(Vec<u8>),
    File(tokio::fs::File),
}

pub(super) fn response(output: CompletedCommand) -> io::Result<Response> {
    let parts: VecDeque<_> = output
        .into_json_parts()?
        .into_iter()
        .map(|part| match part {
            ResponsePart::Bytes(bytes) => Part::Bytes(bytes),
            ResponsePart::File(file) => Part::File(tokio::fs::File::from_std(file)),
        })
        .collect();
    let stream = futures_util::stream::try_unfold(parts, |mut parts| async move {
        loop {
            match parts.front_mut() {
                Some(Part::File(file)) => {
                    let mut chunk = vec![0; 16 * 1024];
                    let read = file.read(&mut chunk).await?;
                    if read == 0 {
                        parts.pop_front();
                        continue;
                    }
                    chunk.truncate(read);
                    return Ok::<_, io::Error>(Some((chunk, parts)));
                }
                Some(Part::Bytes(_)) => {
                    let Some(Part::Bytes(bytes)) = parts.pop_front() else {
                        unreachable!()
                    };
                    return Ok(Some((bytes, parts)));
                }
                None => return Ok(None),
            }
        }
    });
    Response::builder()
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from_stream(stream))
        .map_err(io::Error::other)
}
