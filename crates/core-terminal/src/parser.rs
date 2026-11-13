use vte::{Params, Parser, Perform};

/// A VT parser wrapping `vte::Parser`.
///
/// Feed raw bytes via [`VteParser::advance`]; the actions are dispatched to
/// the inner [`Performer`] which accumulates printable text for consumers.
///
/// This is a scaffold for Bloque 1. In Bloque 5 the performer will be
/// extended to maintain a full cell grid for keyword highlighting.
pub struct VteParser {
    parser: Parser,
    performer: Performer,
}

impl VteParser {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            performer: Performer::default(),
        }
    }

    /// Feed raw bytes to the parser.
    pub fn advance(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.parser.advance(&mut self.performer, *byte);
        }
    }

    /// Drain the accumulated printable text since the last call.
    pub fn drain_text(&mut self) -> String {
        std::mem::take(&mut self.performer.text)
    }
}

impl Default for VteParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal performer — collects printable characters, ignores control ops
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Performer {
    text: String,
}

impl Perform for Performer {
    fn print(&mut self, c: char) {
        self.text.push(c);
    }

    fn execute(&mut self, _byte: u8) {}

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
