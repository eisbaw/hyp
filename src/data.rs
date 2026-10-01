//! Pure helpers for captured data records (decision-0005): the size limit,
//! media types and the text preview the WebUI shows. No I/O here; the store
//! reads and writes the bytes.

/// The most bytes one capture (or attachment) may hold.
pub const LIMIT: usize = 32 * 1024 * 1024;
/// How much of a `text/*` data record the WebUI previews.
pub const PREVIEW_BYTES: usize = 4 * 1024;

/// Whether `s` is a SHA-256 as hyp writes it: 64 lowercase hex digits. The
/// stored bytes live at `hyp/assets/<sha256>`, so this also keeps the name
/// a plain file name.
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The media type of captured bytes when none was given: by the file name's
/// extension for a few common kinds, otherwise `text/plain` for UTF-8 without
/// NUL bytes and `application/octet-stream` for anything else. A guess, good
/// enough to decide whether a preview makes sense; `--media-type` overrides it.
pub fn guess_media_type(name: Option<&str>, bytes: &[u8]) -> String {
    let extension = name
        .and_then(|n| n.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase());
    let known = match extension.as_deref() {
        Some("txt" | "log" | "out" | "err") => Some("text/plain"),
        Some("md" | "markdown") => Some("text/markdown"),
        Some("csv") => Some("text/csv"),
        Some("tsv") => Some("text/tab-separated-values"),
        Some("diff" | "patch") => Some("text/x-diff"),
        Some("json") => Some("application/json"),
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("pdf") => Some("application/pdf"),
        Some("gz") => Some("application/gzip"),
        Some("zip") => Some("application/zip"),
        Some("pcap") => Some("application/vnd.tcpdump.pcap"),
        _ => None,
    };
    let text = !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok();
    known
        .unwrap_or(if text {
            "text/plain"
        } else {
            "application/octet-stream"
        })
        .to_string()
}

/// Whether `s` has the form `type/subtype`, optionally with parameters
/// (`text/plain; charset=utf-8`): printable ASCII, no line breaks.
pub fn is_media_type(s: &str) -> bool {
    let essence = s.split(';').next().unwrap_or_default().trim();
    let token = |t: &str| {
        !t.is_empty()
            && t.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
    };
    s.len() <= 255
        && s.bytes().all(|b| (0x20..0x7f).contains(&b))
        && essence
            .split_once('/')
            .is_some_and(|(t, sub)| token(t) && token(sub))
}

/// The start of `bytes` as text, for a preview: at most `PREVIEW_BYTES`, cut
/// at a character boundary, invalid UTF-8 replaced. The WebUI escapes it.
pub fn preview(bytes: &[u8]) -> String {
    let cut = bytes.len().min(PREVIEW_BYTES);
    let mut end = cut;
    // A cut through a multi-byte character leaves up to 3 of its bytes
    // before it; the byte at the cut is then a continuation byte.
    let continues = |i: usize| bytes.get(i).is_some_and(|b| b & 0xc0 == 0x80);
    while end > cut.saturating_sub(3) && continues(end) {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types_are_guessed_from_the_name_then_the_content() {
        assert_eq!(guess_media_type(Some("run.LOG"), b"\0"), "text/plain");
        assert_eq!(guess_media_type(Some("a.png"), b"x"), "image/png");
        assert_eq!(guess_media_type(None, "tæst\n".as_bytes()), "text/plain");
        assert_eq!(
            guess_media_type(Some("blob"), b"a\0b"),
            "application/octet-stream"
        );
        assert_eq!(
            guess_media_type(None, &[0xff, 0xfe]),
            "application/octet-stream"
        );
    }
    #[test]
    fn media_type_form() {
        for ok in ["text/plain", "text/plain; charset=utf-8", "image/svg+xml"] {
            assert!(is_media_type(ok), "{ok}");
        }
        for bad in ["", "text", "text/", "/plain", "text/pl ain", "text/plain\n"] {
            assert!(!is_media_type(bad), "{bad:?}");
        }
    }
    #[test]
    fn a_preview_stops_at_a_character_boundary() {
        let mut bytes = vec![b'a'; PREVIEW_BYTES - 1];
        bytes.extend("é and more".as_bytes());
        let p = preview(&bytes);
        assert_eq!(p.len(), PREVIEW_BYTES - 1, "the cut 'é' is left out");
        assert!(!p.contains('\u{fffd}'));
        assert_eq!(preview(b"short"), "short");
    }
    #[test]
    fn sha256_form() {
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"A".repeat(64)));
        assert!(!is_sha256("../../etc/passwd"));
    }
}
