//! Transport over the Linux kernel `usbtmc` driver.
//!
//! The kernel exposes each instrument as a character device that takes a SCPI
//! string on write and returns the response on read, so the framing is the
//! driver's problem rather than ours.
//!
//! Devices are addressed through the stable `/dev/rigol-psuN` symlinks created
//! by the udev rule in `deploy/`, never through `/dev/usbtmcN` directly:
//! enumeration order shuffles across reboots and replugs, and with three
//! identical supplies that would silently point a preset at the wrong unit.

use std::ffi::c_int;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::TransportError;
use crate::transport::Transport;

/// Responses are short — the longest is an identity string — but the buffer is
/// generous so a reply is never split across reads.
const READ_BUFFER: usize = 4096;

/// `USBTMC_IOCTL_SET_TIMEOUT` from `linux/usb/tmc.h`, `_IOW(91, 10, __u32)`.
///
/// Encoded by hand to avoid a bindgen dependency for a single constant. This is
/// applied best-effort: if the ioctl is rejected the driver's own default
/// timeout still applies, so a wrong value here degrades to "less responsive",
/// never to "no timeout". Confirm against the target kernel's header — timeout
/// and disconnect semantics vary by kernel version and are a known open risk.
const USBTMC_IOCTL_SET_TIMEOUT: libc::c_ulong = 0x4004_5B0A;

pub struct UsbTmcTransport {
    path: PathBuf,
    file: File,
    timeout: Duration,
}

impl UsbTmcTransport {
    /// Open a supply, applying `timeout` to subsequent reads where the driver
    /// supports it.
    pub fn open(path: impl AsRef<Path>, timeout: Duration) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(map_io_error)?;

        let transport = Self {
            path,
            file,
            timeout,
        };
        transport.apply_timeout();
        Ok(transport)
    }

    fn apply_timeout(&self) {
        let millis = self.timeout.as_millis().min(u32::MAX as u128) as u32;
        // SAFETY: the fd is owned and open, and the ioctl takes a pointer to a
        // u32 that outlives the call.
        let rc: c_int = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                USBTMC_IOCTL_SET_TIMEOUT,
                &millis as *const u32,
            )
        };
        if rc != 0 {
            tracing::debug!(
                path = %self.path.display(),
                error = %std::io::Error::last_os_error(),
                "could not set usbtmc timeout; falling back to the driver default"
            );
        }
    }

    fn send(&mut self, command: &str) -> Result<(), TransportError> {
        let mut line = String::with_capacity(command.len() + 1);
        line.push_str(command.trim_end());
        line.push('\n');
        self.file.write_all(line.as_bytes()).map_err(map_io_error)?;
        self.file.flush().map_err(map_io_error)
    }

    fn receive(&mut self) -> Result<String, TransportError> {
        let mut buf = vec![0u8; READ_BUFFER];
        let n = self.file.read(&mut buf).map_err(|e| {
            // The driver reports a read timeout as ETIMEDOUT, which std maps to
            // TimedOut on the kernels this runs on.
            if e.kind() == ErrorKind::TimedOut {
                TransportError::Timeout(self.timeout)
            } else {
                map_io_error(e)
            }
        })?;
        if n == 0 {
            return Err(TransportError::Timeout(self.timeout));
        }
        String::from_utf8(buf[..n].to_vec())
            .map(|s| s.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|e| TransportError::Protocol(format!("response was not utf-8: {e}")))
    }
}

impl Transport for UsbTmcTransport {
    fn write(&mut self, command: &str) -> Result<(), TransportError> {
        self.send(command)
    }

    fn query(&mut self, command: &str) -> Result<String, TransportError> {
        self.send(command)?;
        self.receive()
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

/// Distinguishes "the cable was pulled" from other I/O failures, because the
/// two lead to different operator-visible states.
fn map_io_error(e: std::io::Error) -> TransportError {
    match e.kind() {
        ErrorKind::NotFound | ErrorKind::BrokenPipe => TransportError::DeviceGone,
        ErrorKind::TimedOut => TransportError::Timeout(Duration::ZERO),
        _ if e.raw_os_error() == Some(libc::ENODEV) => TransportError::DeviceGone,
        _ => TransportError::Io(e),
    }
}
