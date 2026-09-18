//! Read capability replies before Crossterm takes ownership of console input.
use super::{Capabilities, CapabilityDetector, ProbeIo};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HANDLE, WAIT_FAILED, WAIT_TIMEOUT};
use windows_sys::Win32::System::Console::*;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

pub(super) fn detect(timeout: Duration) -> io::Result<Capabilities> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(Capabilities::none());
    }
    let mut console = ConsoleProbe::new()?;
    CapabilityDetector::new(timeout).detect(&mut console)
}

struct ConsoleProbe {
    input: HANDLE,
    output: HANDLE,
    input_mode: u32,
    output_mode: u32,
    saved_input: Vec<INPUT_RECORD>,
}

impl ConsoleProbe {
    fn new() -> io::Result<Self> {
        // SAFETY: borrowed standard handles are used only for console APIs;
        // all out parameters point to initialized, live storage.
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let output = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut input_mode = 0;
            let mut output_mode = 0;
            if GetConsoleMode(input, &mut input_mode) == 0
                || GetConsoleMode(output, &mut output_mode) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let console = Self {
                input,
                output,
                input_mode,
                output_mode,
                saved_input: Vec::new(),
            };
            // Query replies are delivered as console input records even without
            // ENABLE_VIRTUAL_TERMINAL_INPUT. The normal event reader takes over
            // once this bounded startup probe has restored the console modes.
            if SetConsoleMode(
                input,
                input_mode
                    & !(ENABLE_LINE_INPUT
                        | ENABLE_ECHO_INPUT
                        | ENABLE_PROCESSED_INPUT
                        | ENABLE_VIRTUAL_TERMINAL_INPUT),
            ) == 0
                || SetConsoleMode(output, output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(console)
        }
    }
}

impl Drop for ConsoleProbe {
    fn drop(&mut self) {
        // SAFETY: the handles are borrowed, never closed here. The saved
        // records remain alive for the complete synchronous write.
        unsafe {
            if !self.saved_input.is_empty() {
                let mut written = 0;
                WriteConsoleInputW(
                    self.input,
                    self.saved_input.as_ptr(),
                    self.saved_input.len() as u32,
                    &mut written,
                );
            }
            SetConsoleMode(self.input, self.input_mode);
            SetConsoleMode(self.output, self.output_mode);
        }
    }
}

impl ProbeIo for ConsoleProbe {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut output = io::stdout().lock();
        output.write_all(bytes)?;
        output.flush()
    }

    fn receive(&mut self, timeout: Duration) -> io::Result<Vec<u8>> {
        let started = Instant::now();
        loop {
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Ok(Vec::new());
            }
            // SAFETY: only this startup probe reads console input. Waiting is
            // bounded, and the read is limited to already available records.
            unsafe {
                match WaitForSingleObject(
                    self.input,
                    remaining.as_millis().clamp(1, u32::MAX as u128 - 1) as u32,
                ) {
                    WAIT_TIMEOUT => return Ok(Vec::new()),
                    WAIT_FAILED => return Err(io::Error::last_os_error()),
                    _ => {}
                }
                let mut available = 0;
                if GetNumberOfConsoleInputEvents(self.input, &mut available) == 0 {
                    return Err(io::Error::last_os_error());
                }
                if available == 0 {
                    continue;
                }
                let mut records: [INPUT_RECORD; 1024] = std::mem::zeroed();
                let mut count = 0;
                if ReadConsoleInputW(
                    self.input,
                    records.as_mut_ptr(),
                    available.min(records.len() as u32),
                    &mut count,
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                let mut bytes = Vec::new();
                for record in &records[..count as usize] {
                    if record.EventType == KEY_EVENT as u16
                        && record.Event.KeyEvent.uChar.UnicodeChar <= 127
                    {
                        let key = record.Event.KeyEvent;
                        if key.bKeyDown != 0 {
                            // ConPTY versions differ: replies can have ordinary
                            // virtual key codes and repeated characters can be
                            // coalesced. Decode characters, as the Unix probe
                            // does, without assuming synthesized key code zero.
                            let count = usize::from(key.wRepeatCount.max(1))
                                .min(super::MAX_PROBE_RESPONSE_BYTES.saturating_sub(bytes.len()));
                            bytes.extend(std::iter::repeat_n(key.uChar.UnicodeChar as u8, count));
                        }
                    } else {
                        self.saved_input.push(*record);
                    }
                }
                if !bytes.is_empty() {
                    return Ok(bytes);
                }
            }
        }
    }
}
