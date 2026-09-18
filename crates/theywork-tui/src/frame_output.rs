//! Assemble a complete frame before allowing any of its bytes onto the screen.
use std::io::{self, Write};

const BEGIN: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";

pub(crate) struct FrameOutput<W: Write> {
    output: W,
    pending: Option<Vec<u8>>,
    synchronized: bool,
}

impl<W: Write> FrameOutput<W> {
    pub(crate) fn new(output: W, synchronized: bool) -> Self {
        Self {
            output,
            pending: None,
            synchronized,
        }
    }

    pub(crate) fn begin(&mut self) {
        // A view transition may already have queued the previous image's
        // deletion; include it in the first frame of the destination view.
        self.pending.get_or_insert_with(Vec::new);
    }

    pub(crate) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(crate) fn finish(&mut self) -> io::Result<()> {
        let Some(frame) = self.pending.take().filter(|bytes| !bytes.is_empty()) else {
            return Ok(());
        };
        // Encoding has already finished. The terminal's synchronization timeout
        // is spent only on transport, never on rendering or image compression.
        let mut bytes = Vec::with_capacity(frame.len() + BEGIN.len() + END.len());
        if self.synchronized {
            bytes.extend_from_slice(BEGIN);
        }
        bytes.extend_from_slice(&frame);
        if self.synchronized {
            bytes.extend_from_slice(END);
        }
        let result = self
            .output
            .write_all(&bytes)
            .and_then(|()| self.output.flush());
        if result.is_err() && self.synchronized {
            // A partial write may leave the terminal waiting for the frame.
            // ST also terminates an incomplete graphics payload before reset.
            let _ = self.output.write_all(b"\x1b\\\x1b[?2026l");
            let _ = self.output.flush();
        }
        result
    }
}

impl<W: Write> Write for FrameOutput<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(pending) = &mut self.pending {
            pending.extend_from_slice(bytes);
            Ok(bytes.len())
        } else {
            self.output.write(bytes)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_some() {
            Ok(())
        } else {
            self.output.flush()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Sink {
        bytes: Vec<u8>,
        flushes: usize,
        fail_once: bool,
    }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if std::mem::take(&mut self.fail_once) {
                return Err(io::Error::other("injected output failure"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    #[test]
    fn frame_is_invisible_until_complete_with_one_flush() {
        for synchronized in [false, true] {
            let mut output = FrameOutput::new(Sink::default(), synchronized);
            output.begin();
            output.write_all(b"erase").unwrap();
            output.flush().unwrap();
            assert!(output.output.bytes.is_empty());
            output.write_all(b"image text cursor").unwrap();
            output.finish().unwrap();
            let expected = if synchronized {
                b"\x1b[?2026heraseimage text cursor\x1b[?2026l".as_slice()
            } else {
                b"eraseimage text cursor".as_slice()
            };
            assert_eq!(output.output.bytes, expected);
            assert_eq!(output.output.flushes, 1);
            output.begin();
            output.finish().unwrap();
            assert_eq!(output.output.flushes, 1);
        }
    }

    #[test]
    fn canceled_frame_is_discarded_and_write_failure_ends_synchronization() {
        let mut output = FrameOutput::new(Sink::default(), true);
        output.begin();
        output.write_all(b"incomplete").unwrap();
        output.cancel();
        output.flush().unwrap();
        assert!(output.output.bytes.is_empty());
        output.output.fail_once = true;
        output.begin();
        output.write_all(b"frame").unwrap();
        assert!(output.finish().is_err());
        assert!(output.output.bytes.ends_with(END));
    }
}
