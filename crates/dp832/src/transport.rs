use crate::error::TransportError;

/// The single I/O seam of this crate.
///
/// A USBTMC handle is a serialised resource: one command in flight at a time,
/// no interleaving. That constraint is why this trait takes `&mut self` and is
/// deliberately not `Sync` — exclusivity is enforced by ownership, and the
/// server gives each supply its own task that owns its transport.
pub trait Transport: Send {
    /// Send a command that produces no response.
    fn write(&mut self, command: &str) -> Result<(), TransportError>;

    /// Send a query and read its response, with the trailing newline stripped.
    fn query(&mut self, command: &str) -> Result<String, TransportError>;

    /// A short label for logs and error messages, e.g. `/dev/rigol-psu1`.
    fn describe(&self) -> String;
}
