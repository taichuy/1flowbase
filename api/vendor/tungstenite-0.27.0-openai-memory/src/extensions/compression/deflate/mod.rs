//! Implements "permessage-deflate" PMCE defined in [RFC 7692 Section 7]
//!
//! [RFC 7692 Section 7]: https://tools.ietf.org/html/rfc7692#section-7
use bytes::Bytes;
use flate2::{Compress, Decompress, FlushCompress, FlushDecompress, Status};
use thiserror::Error;

use crate::{extensions::compression::DecompressionError, protocol::Role};

mod config;
#[cfg_attr(not(feature = "handshake"), allow(unused_imports))]
pub(crate) use config::ParameterError as DeflateParameterError;
pub use config::{
    DeflateConfig, NegotiationError as DeflateNegotiationError, PermessageDeflateConfig,
    PER_MESSAGE_DEFLATE as EXTENSION_NAME,
};

#[derive(Debug)]
/// Manages per message compression using DEFLATE.
pub struct DeflateContext {
    compress: DeflateCompress,
    decompress: DeflateDecompress,
}

/// Errors from `permessage-deflate` extension.
#[derive(Copy, Clone, Debug, Error, PartialEq, Eq)]
pub enum DeflateError {
    /// Compress failed
    #[error("Failed to compress")]
    Compress,
    /// Decompress failed
    #[error("Failed to decompress")]
    Decompress,
}

#[derive(Debug)]
struct DeflateCompress {
    own_context_takeover: bool,
    /// The actual compressor to run payloads through.
    ///
    /// Use the low-level [`Compress`] API instead of the higher-level
    /// [`flate2::zlib::write::ZlibEncoder`] so we can compress directly into
    /// the output buffer instead of the intermediate one that that type holds.
    compressor: Compress,
}

#[derive(Debug)]
struct DeflateDecompress {
    /// The actual decompressor to run payloads through.
    ///
    /// Use the low-level [`Decompress`] API instead of the higher-level
    /// [`flate2::zlib::write::ZlibDecoder`] so we can decompress directly into
    /// the output buffer instead of the intermediate one that that type holds.
    /// This also lets us avoid some decompression errors that the higher-level
    /// version exhibited with certain highly-compressed payloads.
    decompressor: Decompress,
    peer_context_takeover: bool,
}

impl DeflateContext {
    pub(crate) fn new(role: Role, config: DeflateConfig) -> Self {
        let DeflateConfig {
            server_no_context_takeover,
            client_no_context_takeover,
            compression,
            ..
        } = config;

        // Per RFC 7692 Section 7:
        //
        //      These parameters enable two methods (no_context_takeover and
        //      max_window_bits) of constraining memory usage that may be
        //      applied independently to either direction of WebSocket traffic.
        //      The extension parameters with the "client_" prefix are used by
        //      the client to configure its compressor and by the server to
        //      configure its decompressor.  The extension parameters with the
        //      "server_" prefix are used by the server to configure its
        //      compressor and by the client to configure its decompressor.  All
        //      four parameters are defined for both a client's extension
        //      negotiation offer and a server's extension negotiation response.
        //
        // Here `role` is for our own end of the connection, as opposed to the
        // peer end.
        let (own_no_context_takeover, peer_no_context_takeover) = match role {
            Role::Client => (client_no_context_takeover, server_no_context_takeover),
            Role::Server => (server_no_context_takeover, client_no_context_takeover),
        };

        // Both ends of the connection act as both compressor and decompressor.
        // We compress with the window size for our role and decompress with the
        // size for the opposite role.
        let (compressor_window_bits, decompressor_window_bits) = match role {
            Role::Client => (config.client_max_window_bits(), config.server_max_window_bits()),
            Role::Server => (config.server_max_window_bits(), config.client_max_window_bits()),
        };

        DeflateContext {
            compress: DeflateCompress {
                own_context_takeover: !own_no_context_takeover,
                compressor: Compress::new_with_window_bits(
                    compression,
                    false,
                    compressor_window_bits.get(),
                ),
            },
            decompress: DeflateDecompress {
                peer_context_takeover: !peer_no_context_takeover,
                decompressor: Decompress::new_with_window_bits(
                    false,
                    decompressor_window_bits.get(),
                ),
            },
        }
    }

    /// Compress the payload of an outgoing message.
    pub(crate) fn compress(&mut self, data: &[u8]) -> Result<Bytes, DeflateError> {
        self.compress.compress(data).map_err(|e| {
            log::debug!("compression failed: {e}");
            DeflateError::Compress
        })
    }

    /// Decompress the payload in a received frame.
    ///
    /// The `is_final` argument should only be set when calling with the contents of the last frame in a message.
    pub(crate) fn decompress(
        &mut self,
        data: &[u8],
        is_final: bool,
        size_limit: usize,
    ) -> Result<Bytes, DecompressionError<DeflateError>> {
        self.decompress.decompress(data, is_final, size_limit).map_err(|e| {
            e.map(|e: std::io::Error| {
                log::debug!("decompression failed: {e}");
                DeflateError::Decompress
            })
        })
    }
}

const ELIDED_TRAILER_BLOCK_CONTENTS: &[u8] = &[0x00, 0x00, 0xff, 0xff];

impl DeflateCompress {
    /// Compress the contents of an entire message.
    ///
    /// This is asymmetric with [`DeflateDecompress::decompress`] in that it
    /// operates on the contents of an entire message, not the comprising frames.
    fn compress(&mut self, mut data: &[u8]) -> Result<Bytes, std::io::Error> {
        log::trace!("compressing message payload with {} bytes", data.len());
        if data.is_empty() {
            // Fast path for an empty payload: it gets DEFLATE compressed to a
            // zero-length uncompressed block, which conveniently is
            // concat([0x00], ELIDED_TRAILER_BLOCK_CONTENTS). Then, per the RFC,
            // we elide the trailing 4 bytes to get a single 0x00 byte as the
            // compressed payload.
            return Ok(Bytes::from_static(&[0x00]));
        }

        let mut output = Vec::new();

        // The amount of space that should be available in `output` before
        // attempting to compress data into it.
        const REQUIRED_OUTPUT_SPACE: usize = 4096;

        // Per RFC 7692 Section 7.2.1:
        //
        //     An endpoint uses the following algorithm to compress a message.
        //
        //     1.  Compress all the octets of the payload of the message using
        //         DEFLATE.
        //

        {
            let mut total_read = self.compressor.total_in();
            loop {
                // Make sure there's space for compress_vec to write to.
                output.reserve(REQUIRED_OUTPUT_SPACE);

                let r = self.compressor.compress_vec(data, &mut output, FlushCompress::None)?;

                let read_before = std::mem::replace(&mut total_read, self.compressor.total_in());
                let read = (total_read - read_before) as usize;

                data = &data[read..];
                log::trace!(
                    "compressed {read} bytes, {} remaining; partial output is {} bytes",
                    data.len(),
                    output.len()
                );

                match r {
                    Status::Ok => continue,
                    Status::BufError if read == 0 => {
                        // We made no progress, so this BufError means that
                        // we're out of input.
                        break;
                    }
                    Status::BufError => {
                        // We made some progress, so we can continue after
                        // making more output space.
                        continue;
                    }
                    Status::StreamEnd => break,
                }
            }
        }

        log::trace!("flushing compressed data");

        //     2.  If the resulting data does not end with an empty DEFLATE
        //         block with no compression (the "BTYPE" bits are set to 00),
        //         append an empty DEFLATE block with no compression to the tail
        //         end.

        // Ideally, at this point, we'd be able to just call compress_vec once
        // with an empty slice, FlushCompress::Sync, and a vector with more than
        // enough output space, and then we'd get an empty block and be done.
        // After all, compress_vec is documented to output "as much output as
        // possible".  Unfortunately, compress_vec does not actually do that for
        // all backends. See:
        // - https://github.com/rust-lang/flate2-rs/blob/1.1.2/src/ffi/rust.rs#L169
        // - https://github.com/Frommi/miniz_oxide/blob/0.8.8/miniz_oxide/src/deflate/stream.rs#L82
        // - https://github.com/Frommi/miniz_oxide/issues/105
        //
        // This causes compress_vec to return Ok as soon as the compressor
        // writes *any* output when called with an empty slice.
        //
        // So, instead, we need to keep calling compress_vec with an empty slice
        // until we stop making progress.
        //
        // Once we have done that properly, we should always have an empty block
        // at the end of the output, and then we can truncate the output to
        // remove the empty block, per the RFC.
        {
            let mut total_out = self.compressor.total_out();
            loop {
                output.reserve(REQUIRED_OUTPUT_SPACE);
                let output_len_before = output.len();
                let output_available_before = output.capacity() - output_len_before;

                let _ = self.compressor.compress_vec(&[], &mut output, FlushCompress::Sync)?;
                log::trace!(
                    "flushed {} bytes into an available {output_available_before} bytes",
                    output.len() - output_len_before,
                );
                let out_before = std::mem::replace(&mut total_out, self.compressor.total_out());
                if total_out == out_before {
                    break;
                }
            }
        }

        //     3.  Remove 4 octets (that are 0x00 0x00 0xff 0xff) from the tail
        //         end.  After this step, the last octet of the compressed data
        //         contains (possibly part of) the DEFLATE header bits with the
        //         "BTYPE" bits set to 00.

        debug_assert!(output.ends_with(ELIDED_TRAILER_BLOCK_CONTENTS), "output is {output:02x?}");
        output.truncate(output.len() - ELIDED_TRAILER_BLOCK_CONTENTS.len());

        if !self.own_context_takeover {
            // Reset if the next frame isn't supposed to be starting with the
            // same compression window.
            self.compressor.reset();
        }

        log::trace!("finished compression into {} bytes", output.len());
        Ok(Bytes::from(output))
    }
}

impl DeflateDecompress {
    /// Decompress the contents of a single frame.
    ///
    /// The `is_final` argument must be `true` if and only if the frame is the
    /// last one in a message. The `size_limit` argument is the maximum number
    /// of bytes that can be decompressed. If the input `data` decompresses to
    /// more than `size_limit` bytes, [`DecompressionError::SizeLimitReached`]
    /// will be returned.
    fn decompress(
        &mut self,
        data: &[u8],
        is_final: bool,
        size_limit: usize,
    ) -> Result<Bytes, DecompressionError<std::io::Error>> {
        // From RFC 7692 Section 7.2.2:
        //
        //   An endpoint uses the following algorithm to decompress a message.
        //
        //   1.  Append 4 octets of 0x00 0x00 0xff 0xff to the tail end of the
        //       payload of the message.
        //
        //   2.  Decompress the resulting data using DEFLATE.

        let mut output = Vec::new();

        log::trace!(
            "decompressing {} bytes in {} frame",
            data.len(),
            if is_final { "final" } else { "intermediate" }
        );
        let mut total_read = self.decompressor.total_in();

        let mut decompress_from = |mut data: &[u8]| {
            loop {
                // Make sure there's some space to decompress into,
                // optimistically assuming a 50% compression ratio of the input.
                // This might put us slightly beyond the requested size limit
                // but it also might not all be used.
                output.reserve(2 * data.len());

                let r =
                    self.decompressor.decompress_vec(data, &mut output, FlushDecompress::None)?;

                if output.len() > size_limit {
                    return Err(DecompressionError::SizeLimitReached);
                }
                let read_before = std::mem::replace(&mut total_read, self.decompressor.total_in());

                let read = (total_read - read_before) as usize;

                data = &data[read..];

                match r {
                    Status::Ok => continue,
                    Status::BufError => {
                        // We've either run out of input data or output space.
                        // Since we reserve space ahead of time, this must mean
                        // we're out of input.
                        break;
                    }
                    Status::StreamEnd => {
                        // Finished a block with BFINAL set. This is legal; from
                        // RFC 7692 Section 7.2.3.4:
                        //
                        //   On platforms on which the flush method using an
                        //   empty DEFLATE block with no compression is not
                        //   available, implementors can choose to flush data
                        //   using DEFLATE blocks with "BFINAL" set to 1.
                        //
                        // On the decompression end we reset the compressor in
                        // response. This relies on the assumption that the
                        // client produced the block with BFINAL set by
                        // informing their compressor that the stream was
                        // ending, and so any blocks afterwards won't reference
                        // any context from this block or earlier. It's
                        // obviously not a perfect assumption, but it matches
                        // the behavior of other widely-deployed
                        // permessage-deflate implementations.
                        self.decompressor.reset(false);
                        total_read = 0;
                    }
                }
            }
            Ok(())
        };

        decompress_from(data)?;

        if is_final {
            // Decompress the final block that is part of the logical input to
            // DEFLATE but is elided from the message payloads. This implicitly
            // flushes out any pending bytes that were part of the previous
            // block and doesn't leave any others since the trailer is explicitly
            // an empty block.
            decompress_from(&ELIDED_TRAILER_BLOCK_CONTENTS)?;

            if !self.peer_context_takeover {
                self.decompressor.reset(false);
            }
        }

        Ok(Bytes::from(output))
    }
}

impl From<DeflateContext> for super::PerMessageCompressionContext {
    fn from(value: DeflateContext) -> Self {
        Self::Deflate(value)
    }
}

#[cfg(test)]
pub(crate) mod test {
    use rand::{distr::Distribution as _, RngCore, SeedableRng as _};

    use super::*;

    #[test]
    fn interop() {
        let mut data = vec![0; 2048];
        rand::rngs::SmallRng::seed_from_u64(1023).fill_bytes(&mut data);

        let configs = [
            DeflateConfig::default(),
            DeflateConfig::default().set_no_context_takeover(Role::Client, true),
            DeflateConfig::default()
                .set_no_context_takeover(Role::Client, true)
                .set_max_window_bits(Role::Client, 10)
                .unwrap(),
            DeflateConfig::default().set_max_window_bits(Role::Client, 10).unwrap(),
        ];

        let frame_sizes = [16, 64, data.len()];

        for config in configs {
            for frame_size in frame_sizes {
                let mut client = DeflateContext::new(Role::Client, config);
                let mut server = DeflateContext::new(Role::Server, config);

                let mut send_and_receive = |data| {
                    let compressed = client.compress(data).unwrap();

                    let mut decompressed = Vec::<u8>::new();

                    let mut it = compressed.chunks(frame_size).peekable();
                    while let Some(frame) = it.next() {
                        decompressed.extend_from_slice(
                            &server.decompress(frame, it.peek().is_none(), usize::MAX).unwrap(),
                        );
                    }
                    decompressed
                };

                let decompressed = send_and_receive(&data);
                assert_eq!(data, decompressed);

                // Make sure we haven't broken compression or decompression for
                // the *next* message.
                let decompressed = send_and_receive(b"second message");
                assert_eq!(decompressed, b"second message");
            }
        }
    }

    #[test]
    fn large_message_compression() {
        let mut data = vec![0; 1 << 19];
        rand::rngs::SmallRng::seed_from_u64(1023).fill_bytes(&mut data);

        let mut context = DeflateContext::new(Role::Client, DeflateConfig::default());

        let compressed = context.compress(&data).unwrap();

        assert_eq!(&context.decompress(&compressed, true, usize::MAX).unwrap(), &data);
    }

    #[test]
    fn decompression_limits_applied() {
        let data = vec![0; 1 << 18];

        let mut context = DeflateContext::new(Role::Client, DeflateConfig::default());
        let compressed = context.compress(&data).unwrap();

        // A buffer of all zeros compresses very well.
        assert!(compressed.len() < data.len() / 500);

        assert_eq!(
            context.decompress(&compressed, true, data.len() - 1),
            Err(DecompressionError::SizeLimitReached)
        );
    }

    #[test]
    fn compressible_payload_prefixes() {
        let _ = env_logger::try_init();
        let data: Vec<u8> = rand::distr::Alphanumeric
            .sample_iter(&mut rand::rngs::SmallRng::from_seed([59; 32]))
            .take(1 << 16)
            .collect();

        let prefixes =
            (5..).map(|i| 1 << i).take_while(|len| *len <= data.len()).map(|len| &data[..len]);

        for prefix in prefixes {
            let mut context = DeflateContext::new(Role::Client, DeflateConfig::default());
            println!("compressing {} bytes of compressible data", prefix.len());

            let compressed = context.compress(prefix).unwrap();
            assert_eq!(context.decompress(&compressed, true, usize::MAX).unwrap(), prefix);
        }
    }

    /// Utilities for testing decomrpession of highly-compressed payloads.
    pub(crate) mod very_compressed {
        use bytes::Bytes;

        // Compressed payload that decompresses to 50KB of zeroes. This was
        // specifically chosen so that its compressed form aligns with a byte
        // boundary, which lets us repeat it an arbitrary number of times to
        // form the payload of a single message.
        pub(crate) const FRAME_PAYLOAD: &[u8; 66] = &[
            0xec, 0xc1, 0x31, 0x01, 0x00, 0x00, 0x00, 0xc2, 0xa0, 0xf5, 0x4f, 0x6d, 0x0b, 0x2f,
            0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x6f,
        ];
        pub(crate) const DECOMPRESSED_LEN: usize = 50 * 1024;

        pub(crate) fn make_frames(frame_count: usize) -> impl Iterator<Item = (Bytes, bool)> {
            std::iter::repeat_n(FRAME_PAYLOAD, frame_count).enumerate().map(move |(i, bytes)| {
                let is_final = i == frame_count - 1;
                let bytes = if is_final {
                    bytes.iter().copied().chain(std::iter::once(0x00)).collect()
                } else {
                    Bytes::from_static(bytes)
                };
                (bytes, is_final)
            })
        }
    }

    #[test]
    fn large_message_decompression() {
        let _ = env_logger::try_init();

        for frame_count in 1..=10 {
            let mut context = DeflateContext::new(Role::Client, DeflateConfig::default());

            let decompressed: Bytes = very_compressed::make_frames(frame_count)
                .enumerate()
                .flat_map(|(i, (frame, is_final))| {
                    context
                        .decompress
                        .decompress(&frame, is_final, usize::MAX)
                        .unwrap_or_else(|e| panic!("deflating frame {i}/{frame_count} failed: {e}"))
                })
                .collect();
            assert!(decompressed.iter().all(|b| *b == 0));
            assert_eq!(decompressed.len(), frame_count * very_compressed::DECOMPRESSED_LEN);
        }
    }

    #[test]
    fn decompress_multiple_messages_that_each_set_bfinal() {
        let _ = env_logger::try_init();

        let mut rng = rand::rngs::SmallRng::from_seed([12; 32]);
        let uncompressed_payloads = std::iter::repeat_with(|| {
            let mut data: Vec<u8> = vec![0; 1 << 12];
            rng.fill_bytes(&mut data);
            data
        });

        let mut context = DeflateContext::new(Role::Server, DeflateConfig::default());

        for (i, payload) in uncompressed_payloads.enumerate().take(5) {
            let mut compressed = context.compress(&payload).unwrap().try_into_mut().unwrap();
            // The final block in the stream is a 5-byte uncompressed block, but
            // with the trailing 4 bytes of the body chopped off (per the RFC).
            // We don't know where in the last *byte* the final block begins
            // (since DEFLATE is a bit-oriented protocol), so to make sure the
            // payload ends with a block with BFINAL set we need to append
            // another block. First we reattach the chopped-off bytes from the
            // last block. Then we push *another* 5-byte uncompressed block with
            // BFINAL set. Lastly we chop off the trailing 4 bytes per the spec.
            compressed.extend_from_slice(ELIDED_TRAILER_BLOCK_CONTENTS);
            compressed.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
            compressed.truncate(compressed.len() - ELIDED_TRAILER_BLOCK_CONTENTS.len());

            println!("decompressing block {i}");
            let decompressed = context.decompress(&compressed, true, usize::MAX).unwrap();
            assert_eq!(decompressed.len(), payload.len());
            assert_eq!(decompressed, payload);
        }
    }

    mod rfc_7692_section_7_2_3_examples {
        use super::*;

        #[test]
        fn one_block() {
            // From RFC 7692 Section 7.2.3.1:
            //
            //   Suppose that an endpoint sends a text message "Hello".  If the
            //   endpoint uses one compressed DEFLATE block (compressed with fixed
            //   Huffman code and the "BFINAL" bit not set) to compress the message,
            //   the endpoint obtains the compressed data to use for the message
            //   payload as follows.
            //
            //   The endpoint compresses "Hello" into one compressed DEFLATE block and
            //   flushes the resulting data into a byte array using an empty DEFLATE
            //   block with no compression:
            //
            //       0xf2 0x48 0xcd 0xc9 0xc9 0x07 0x00 0x00 0x00 0xff 0xff
            //
            //   By stripping 0x00 0x00 0xff 0xff from the tail end, the endpoint gets
            //   the data to use for the message payload:
            //
            const EXPECTED_COMPRESSED_PAYLOAD: &[u8] = &[0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];

            let mut context = DeflateContext::new(Role::Server, DeflateConfig::default());
            let compressed = context.compress(b"Hello").unwrap();
            assert_eq!(&compressed[..], EXPECTED_COMPRESSED_PAYLOAD);
            //
            //   ...
            //
            //   Suppose that the endpoint sends the compressed message with
            //   fragmentation.  The endpoint splits the compressed data into
            //   fragments and builds frames for each fragment.  For example, if the
            //   fragments are 3 and 4 octets,
            //
            const FRAGMENTED_FRAMES: &[&[u8]] = &[
                //  the first frame is:
                &[0x41, 0x03, 0xf2, 0x48, 0xcd],
                //   and the second frame is:
                &[0x80, 0x04, 0xc9, 0xc9, 0x07, 0x00],
            ];
            //
            //   Note that the RSV1 bit is set only on the first frame.

            let frame_payloads =
                FRAGMENTED_FRAMES.iter().map(|frame| &frame[2..]).collect::<Vec<_>>();

            let decompressed = frame_payloads
                .iter()
                .enumerate()
                .map(|(index, payload)| {
                    context.decompress(payload, index == frame_payloads.len() - 1, usize::MAX)
                })
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .concat();

            assert_eq!(decompressed, b"Hello");
        }

        #[test]
        fn sharing_sliding_window() {
            const ROLE: Role = Role::Client;

            // From RFC 7692 Section 7.2.3.2:
            //
            //   Suppose that a client has sent a message "Hello" as a compressed
            //   message and will send the same message "Hello" again as a compressed
            //   message.
            //
            const FIRST_PAYLOAD: &[u8] = &[0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];
            //
            //   The above is the payload of the first message that the client has
            //   sent.  If the "agreed parameters" contain the
            //   "client_no_context_takeover" extension parameter, the client
            //   compresses the payload of the next message into the same bytes (if
            //   the client uses the same "BTYPE" value and "BFINAL" value).  So, the
            //   payload of the second message will be:
            //
            const SECOND_PAYLOAD: &[u8] = &[0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];

            let mut context = DeflateContext::new(
                ROLE,
                DeflateConfig::default().set_no_context_takeover(ROLE, true),
            );
            assert_eq!(&context.compress(b"Hello").unwrap()[..], FIRST_PAYLOAD);
            assert_eq!(&context.compress(b"Hello").unwrap()[..], SECOND_PAYLOAD);

            //
            //   If the "agreed parameters" did not contain the
            //   "client_no_context_takeover" extension parameter, the client can
            //   compress the payload of the next message into fewer bytes by
            //   referencing the history in the LZ77 sliding window.  So, the payload
            //   of the second message will be:
            //
            const NEW_SECOND_PAYLOAD: &[u8] = &[0xf2, 0x00, 0x11, 0x00, 0x00];

            let mut context = DeflateContext::new(ROLE, DeflateConfig::default());
            assert_eq!(&context.compress(b"Hello").unwrap()[..], FIRST_PAYLOAD);
            assert_eq!(&context.compress(b"Hello").unwrap()[..], NEW_SECOND_PAYLOAD);
        }

        #[test]
        fn deflate_block_with_bfinal_set() {
            // From RFC 7692 Section 7.2.3.4:
            //
            //   On platforms on which the flush method using an empty DEFLATE
            //   block with no compression is not available, implementors can
            //   choose to flush data using DEFLATE blocks with "BFINAL" set to
            //   1.

            const PAYLOAD: &[u8] = &[0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x00];

            //   This is the payload of a message containing "Hello" compressed
            //   using a DEFLATE block with "BFINAL" set to 1.  The first 7
            //   octets constitute a DEFLATE block with "BFINAL" set to 1 and
            //   "BTYPE" set to 01 containing "Hello".  The last 1 octet (0x00)
            //   contains the header bits with "BFINAL" set to 0 and "BTYPE" set
            //   to 00, and 5 padding bits of 0.  This octet is necessary to
            //   allow the payload to be decompressed in the same manner as
            //   messages flushed using DEFLATE blocks with "BFINAL" unset.

            let mut context = DeflateContext::new(Role::Client, DeflateConfig::default());
            assert_eq!(
                context.decompress(PAYLOAD, true, usize::MAX),
                Ok(Bytes::from_static(b"Hello"))
            );
        }

        #[test]
        fn two_deflate_blocks() {
            // From RFC 7692 Section 7.2.3.5:
            //
            //   Two or more DEFLATE blocks may be used in one message.

            const TWO_BLOCKS: &[u8] =
                &[0xf2, 0x48, 0x05, 0x00, 0x00, 0x00, 0xff, 0xff, 0xca, 0xc9, 0xc9, 0x07, 0x00];

            let mut context = DeflateContext::new(Role::Client, DeflateConfig::new());

            assert_eq!(&context.decompress(TWO_BLOCKS, true, usize::MAX).unwrap()[..], b"Hello");
        }
    }
}
