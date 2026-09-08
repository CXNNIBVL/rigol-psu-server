use std::time::Duration;

/// A failure at the I/O seam.
///
/// The variants are deliberately coarse and map onto the operational states the
/// server surfaces to users: a timeout or a vanished device takes a supply
/// offline, whereas a protocol error means we are talking to something that is
/// not the instrument we expect.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The instrument did not answer within the configured timeout.
    #[error("timed out after {0:?} waiting for a response")]
    Timeout(Duration),

    /// The device node disappeared — typically the USB cable was pulled.
    #[error("device is no longer present")]
    DeviceGone,

    /// Any other I/O failure against the device node.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// The response was not valid UTF-8, or otherwise unintelligible as SCPI.
    #[error("protocol error: {0}")]
    Protocol(String),
}

impl TransportError {
    /// Whether the supply should be considered offline as a result of this error.
    ///
    /// Protocol errors are excluded: the link is fine, we simply did not
    /// understand the answer, and dropping the whole supply offline would hide
    /// the actual bug.
    pub fn is_disconnect(&self) -> bool {
        matches!(
            self,
            TransportError::Timeout(_) | TransportError::DeviceGone | TransportError::Io(_)
        )
    }
}

/// A response that did not match the shape the command's reply should have.
#[derive(Debug, thiserror::Error)]
#[error("could not parse response to `{command}`: {reason} (got {response:?})")]
pub struct ScpiError {
    pub command: String,
    pub response: String,
    pub reason: String,
}

impl ScpiError {
    pub(crate) fn new(
        command: impl Into<String>,
        response: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            response: response.into(),
            reason: reason.into(),
        }
    }
}

impl From<ScpiError> for TransportError {
    fn from(e: ScpiError) -> Self {
        TransportError::Protocol(e.to_string())
    }
}
