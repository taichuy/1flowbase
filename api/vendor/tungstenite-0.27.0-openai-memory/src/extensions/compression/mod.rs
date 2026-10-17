//! [Per-Message Compression Extensions][rfc7692]
//!
//! [rfc7692]: https://tools.ietf.org/html/rfc7692

use bytes::Bytes;
use thiserror::Error;

#[cfg(feature = "deflate")]
pub mod deflate;

/// Active context for performing per-message compression.
#[derive(Debug)]
#[cfg_attr(not(feature = "deflate"), allow(missing_copy_implementations))] // This is only trivially copyable if compression is disabled.
pub enum PerMessageCompressionContext {
    /// Context for compressing/decompressing with `permessage-deflate`.
    #[cfg(feature = "deflate")]
    Deflate(deflate::DeflateContext),
}

/// Error encountered while compressing or decompressing.
#[derive(Copy, Clone, Debug, Error, PartialEq, Eq)]
pub enum CompressionError {
    /// Error encountered while deflating or inflating
    #[error("Deflate error: {0}")]
    #[cfg(feature = "deflate")]
    Deflate(deflate::DeflateError),
}

#[derive(Debug, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum DecompressionError<E = CompressionError> {
    /// The decompressed frame is larger than the configured limit.
    #[error("decompressed data is too large")]
    SizeLimitReached,
    /// An error was encountered while decompressing.
    #[error("{0}")]
    Decompression(E),
}

impl PerMessageCompressionContext {
    #[inline]
    pub(crate) fn compressor<'s>(
        &'s mut self,
    ) -> impl FnMut(&Bytes) -> Result<Bytes, CompressionError> + 's {
        move |payload: &Bytes| match self {
            #[cfg(feature = "deflate")]
            Self::Deflate(deflate_config) => {
                deflate_config.compress(payload).map_err(CompressionError::Deflate)
            }
            #[cfg(not(feature = "deflate"))]
            _ => {
                let _ = payload;
                unreachable!("*PerMessageCompressionContext is uninhabited")
            }
        }
    }

    #[inline]
    pub(crate) fn decompressor<'s>(
        &'s mut self,
    ) -> impl FnMut(&Bytes, bool, usize) -> Result<Bytes, DecompressionError> + 's {
        move |payload, is_final, size_limit| match self {
            #[cfg(feature = "deflate")]
            Self::Deflate(deflate_config) => deflate_config
                .decompress(payload, is_final, size_limit)
                .map_err(|e| e.map(CompressionError::Deflate)),
            #[cfg(not(feature = "deflate"))]
            _ => {
                let _ = (payload, is_final, size_limit);
                unreachable!("*PerMessageCompressionContext is uninhabited")
            }
        }
    }
}

impl<E> DecompressionError<E> {
    pub(crate) fn map<T>(self, f: impl FnOnce(E) -> T) -> DecompressionError<T> {
        match self {
            Self::SizeLimitReached => DecompressionError::SizeLimitReached,
            Self::Decompression(e) => DecompressionError::Decompression(f(e)),
        }
    }
}

impl<E: Into<std::io::Error>> From<E> for DecompressionError<std::io::Error> {
    fn from(value: E) -> Self {
        Self::Decompression(value.into())
    }
}
