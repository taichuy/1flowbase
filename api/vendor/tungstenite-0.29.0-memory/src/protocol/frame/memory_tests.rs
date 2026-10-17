//! Ownership regressions for the actual codec, without allocator/RSS assumptions.
use super::{
    coding::{Data, OpCode},
    Frame, FrameCodec,
};
use crate::Error;
use std::{
    collections::VecDeque,
    io::{self, Cursor, Read, Write},
};

fn binary(payload: Vec<u8>) -> Frame {
    Frame::message(payload, OpCode::Data(Data::Binary), true)
}

fn wire(payload: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    binary(payload.to_vec()).format(&mut encoded).unwrap();
    encoded
}

#[test]
fn completed_large_frame_detaches_even_when_remaining_capacity_is_small() {
    let payload = vec![0x5a; 32 * 1024];
    let encoded = wire(&payload);
    // Start with just the extended header. The body reservation grows backing
    // to the exact payload length; splitting the completed payload hides it in
    // a zero-capacity remaining view. A capacity-only idle check misses this.
    let mut codec = FrameCodec::from_partially_read(encoded[..4].to_vec(), 64);
    let mut stream = Cursor::new(encoded[4..].to_vec());
    let held = codec.read_frame(&mut stream, None, false, true).unwrap().unwrap();
    let payload_end = held.payload().as_ptr() as usize + payload.len();
    assert_eq!(codec.in_buffer.capacity(), 64);
    assert_ne!(codec.in_buffer.as_ptr() as usize, payload_end);
    // Keep the old allocation live while using the new independent owner.
    assert_eq!(held.into_payload().as_ref(), payload.as_slice());
}

#[test]
fn partially_read_large_backing_defers_copy_until_tail_fits_and_preserves_fifo() {
    let first = vec![0x61; 4096];
    let second = vec![0x62; 4096];
    let first_wire = wire(&first);
    let second_wire = wire(&second);
    let mut encoded = first_wire.clone();
    encoded.extend_from_slice(&second_wire);
    encoded.extend_from_slice(&wire(b"tail"));
    encoded.push(0x82); // An incomplete next header must survive detachment.
    encoded.reserve(first.len() + second.len() + 64);
    let mut codec = FrameCodec::from_partially_read(encoded, 64);
    let original = codec.in_buffer.as_ptr() as usize;
    let held_first = codec.read_frame(&mut io::empty(), None, false, true).unwrap().unwrap();
    // A still-large unread tail is not copied once per completed frame.
    assert_eq!(codec.in_buffer.as_ptr() as usize, original + first_wire.len());
    let held_second = codec.read_frame(&mut io::empty(), None, false, true).unwrap().unwrap();
    assert_eq!(codec.in_buffer.capacity(), 64);
    assert_ne!(codec.in_buffer.as_ptr() as usize, original + first_wire.len() + second_wire.len());
    let tail = codec.read_frame(&mut io::empty(), None, false, true).unwrap().unwrap();
    assert_eq!(tail.into_payload().as_ref(), b"tail");
    assert!(codec.read_frame(&mut io::empty(), None, false, true).unwrap().is_none());
    assert_eq!(&codec.in_buffer[..], &[0x82]);
    assert_eq!(held_first.into_payload().as_ref(), first.as_slice());
    assert_eq!(held_second.into_payload().as_ref(), second.as_slice());
}

#[test]
fn partially_read_small_frame_with_large_spare_backing_detaches() {
    let mut encoded = Vec::with_capacity(32 * 1024);
    encoded.extend_from_slice(&wire(b"held"));
    encoded.push(0x82);
    let mut codec = FrameCodec::from_partially_read(encoded, 64);
    assert!(codec.in_buffer.capacity() > 64);
    let original = codec.in_buffer.as_ptr() as usize;
    let held = codec.read_frame(&mut io::empty(), None, false, true).unwrap().unwrap();
    assert_eq!(codec.in_buffer.capacity(), 64);
    assert_ne!(codec.in_buffer.as_ptr() as usize, original + wire(b"held").len());
    assert_eq!(&codec.in_buffer[..], &[0x82]);
    assert_eq!(held.into_payload().as_ref(), b"held");
}

#[test]
fn ordinary_frames_keep_normal_backing() {
    let mut encoded = wire(b"one");
    encoded.extend_from_slice(&wire(b"two"));
    let mut codec = FrameCodec::from_partially_read(encoded, 64);
    let original = codec.in_buffer.as_ptr() as usize;
    let held = codec.read_frame(&mut io::empty(), None, false, true).unwrap().unwrap();
    assert_eq!(codec.in_buffer.as_ptr() as usize, original + wire(b"one").len());
    assert_eq!(
        codec
            .read_frame(&mut io::empty(), None, false, true)
            .unwrap()
            .unwrap()
            .into_payload()
            .as_ref(),
        b"two"
    );
    assert_eq!(held.into_payload().as_ref(), b"one");
}

struct ReadSteps(VecDeque<Option<Cursor<Vec<u8>>>>);

impl Read for ReadSteps {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.0.front_mut() {
                Some(Some(chunk)) => {
                    let count = chunk.read(output)?;
                    if count != 0 {
                        return Ok(count);
                    }
                    self.0.pop_front();
                }
                Some(None) => {
                    self.0.pop_front();
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                None => return Ok(0),
            }
        }
    }
}

fn would_block(result: Result<Option<Frame>, Error>) {
    assert!(matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock));
}

#[test]
fn partial_extended_header_and_body_survive_would_block() {
    let payload: Vec<u8> = (0..513).map(|i| (i % 251) as u8).collect();
    let encoded = wire(&payload);
    let mut stream = ReadSteps(VecDeque::from([
        Some(Cursor::new(encoded[..1].to_vec())),
        None,
        Some(Cursor::new(encoded[1..3].to_vec())),
        None,
        Some(Cursor::new(encoded[3..200].to_vec())),
        None,
        Some(Cursor::new(encoded[200..].to_vec())),
    ]));
    let mut codec = FrameCodec::new(64);
    would_block(codec.read_frame(&mut stream, None, false, true));
    would_block(codec.read_frame(&mut stream, None, false, true));
    would_block(codec.read_frame(&mut stream, None, false, true));
    let held = codec.read_frame(&mut stream, None, false, true).unwrap().unwrap();
    assert_eq!(codec.in_buffer.capacity(), 64);
    assert_eq!(held.into_payload().as_ref(), payload.as_slice());
}

#[test]
fn streamed_large_frame_keeps_held_bytes_valid_after_small_frame() {
    let payload = vec![0xa5; 32 * 1024];
    let mut encoded = wire(&payload);
    encoded.extend_from_slice(&wire(b"next"));
    let mut stream = Cursor::new(encoded);
    let mut codec = FrameCodec::new(64);
    let held = codec.read_frame(&mut stream, None, false, true).unwrap().unwrap();
    assert_eq!(codec.in_buffer.capacity(), 64);
    assert_eq!(
        codec.read_frame(&mut stream, None, false, true).unwrap().unwrap().into_payload().as_ref(),
        b"next"
    );
    assert_eq!(held.into_payload().as_ref(), payload.as_slice());
}

struct BudgetWriter {
    budget: usize,
    bytes: Vec<u8>,
    error: io::ErrorKind,
    flushes: usize,
}

impl Write for BudgetWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.budget == 0 {
            return Err(self.error.into());
        }
        let count = bytes.len().min(self.budget);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.budget -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

#[test]
fn queued_remainder_survives_partial_write_and_retry_for_both_targets() {
    for target in [0, 64] {
        for error in [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted] {
            let payload = vec![0x39; 8192];
            let mut expected = wire(&payload);
            let mut codec = FrameCodec::new(64);
            codec.set_out_buffer_write_len(target);
            let mut stream = BudgetWriter { budget: 17, bytes: Vec::new(), error, flushes: 0 };
            assert!(
                matches!(codec.buffer_frame(&mut stream, binary(payload)), Err(Error::Io(e)) if e.kind() == error)
            );
            assert_eq!(stream.bytes, expected[..17]);
            assert_eq!(codec.out_buffer, expected[17..]);
            let retained_capacity = codec.out_buffer.capacity();
            assert!(retained_capacity > target);
            expected.extend_from_slice(&wire(b"queued"));
            assert!(codec.buffer_frame(&mut stream, binary(b"queued".to_vec())).is_err());
            assert_eq!(codec.out_buffer, expected[17..]);
            stream.budget = usize::MAX;
            codec.write_out_buffer(&mut stream).unwrap();
            assert_eq!(stream.bytes, expected);
            assert!(codec.out_buffer.is_empty());
            assert_eq!(codec.out_buffer.capacity(), target);
            codec.write_out_buffer(&mut stream).unwrap();
            assert_eq!(stream.bytes, expected); // No duplicate on retry.
            assert_eq!(stream.flushes, 0); // Socket write is distinct from transport flush.
        }
    }
}

#[test]
fn fully_drained_large_write_reclaims_and_write_zero_keeps_queue() {
    for target in [0, 64] {
        let payload = vec![0x77; 8192];
        let expected = wire(&payload);
        let mut codec = FrameCodec::new(64);
        codec.set_out_buffer_write_len(target);
        let mut written = Vec::new();
        codec.buffer_frame(&mut written, binary(payload.clone())).unwrap();
        assert_eq!(written, expected);
        assert_eq!(codec.out_buffer.capacity(), target);
        // Use a writer returning Ok(0), rather than an error after accepting bytes.
        struct Zero;
        impl Write for Zero {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Ok(0)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(codec.buffer_frame(&mut Zero, binary(payload)).is_err());
        assert_eq!(codec.out_buffer, expected);
        assert!(codec.out_buffer.capacity() > target);
        let mut recovered = Vec::new();
        codec.write_out_buffer(&mut recovered).unwrap();
        assert_eq!(recovered, expected);
        assert_eq!(codec.out_buffer.capacity(), target);
    }
}
