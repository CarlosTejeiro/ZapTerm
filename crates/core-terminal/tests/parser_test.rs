use core_terminal::VteParser;

#[test]
fn strips_ansi_escape_sequences() {
    let mut p = VteParser::new();
    // "\x1b[32mOK\x1b[0m" → printable text is just "OK"
    p.advance(b"\x1b[32mOK\x1b[0m");
    assert_eq!(p.drain_text(), "OK");
}

#[test]
fn plain_text_passes_through() {
    let mut p = VteParser::new();
    p.advance(b"hello world");
    assert_eq!(p.drain_text(), "hello world");
}

#[test]
fn drain_clears_buffer() {
    let mut p = VteParser::new();
    p.advance(b"first");
    let _ = p.drain_text();
    p.advance(b"second");
    assert_eq!(p.drain_text(), "second");
}
