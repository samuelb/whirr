//! Transparent ICY (SHOUTcast/Icecast) metadata de-multiplexer.
//!
//! Internet radio streams that advertise `icy-metaint: N` interleave a metadata
//! block after every `N` bytes of audio. [`IcyReader`] wraps the raw stream and
//! implements [`Read`], yielding only the audio bytes while parsing each
//! metadata block and reporting the current `StreamTitle` through a callback.

use std::io::{self, Read};

/// Callback invoked whenever the stream title changes (`None` clears it).
pub type TitleCallback = Box<dyn FnMut(Option<String>) + Send>;

/// A [`Read`] adapter that strips ICY metadata and reports track titles.
pub struct IcyReader<R> {
    inner: R,
    metaint: usize,
    remaining: usize,
    on_title: TitleCallback,
    last_title: Option<String>,
}

impl<R: Read> IcyReader<R> {
    /// Wrap `inner`. If `metaint` is 0 the stream carries no metadata and bytes
    /// are passed through unchanged.
    pub fn new(inner: R, metaint: usize, on_title: TitleCallback) -> Self {
        Self {
            inner,
            metaint,
            remaining: metaint,
            on_title,
            last_title: None,
        }
    }

    fn read_metadata_block(&mut self) -> io::Result<()> {
        let mut len_byte = [0u8; 1];
        self.inner.read_exact(&mut len_byte)?;
        let len = len_byte[0] as usize * 16;
        if len == 0 {
            return Ok(());
        }
        let mut block = vec![0u8; len];
        self.inner.read_exact(&mut block)?;

        if let Some(title) = parse_stream_title(&block) {
            let next = if title.is_empty() { None } else { Some(title) };
            if next != self.last_title {
                self.last_title = next.clone();
                (self.on_title)(next);
            }
        }
        Ok(())
    }
}

impl<R: Read> Read for IcyReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.metaint == 0 {
            return self.inner.read(buf);
        }
        if self.remaining == 0 {
            self.read_metadata_block()?;
            self.remaining = self.metaint;
        }
        let want = buf.len().min(self.remaining);
        let n = self.inner.read(&mut buf[..want])?;
        self.remaining -= n;
        Ok(n)
    }
}

/// Extract the value of `StreamTitle='...'` from a raw ICY metadata block.
fn parse_stream_title(block: &[u8]) -> Option<String> {
    const KEY: &str = "StreamTitle='";
    let text = String::from_utf8_lossy(block);
    let start = text.find(KEY)? + KEY.len();
    let rest = &text[start..];
    // The value ends at the closing `';` (or the end of the block).
    let end = rest.find("';").unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('\0').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title() {
        let block = b"StreamTitle='Artist - Song';StreamUrl='https://example.com/';\0\0";
        assert_eq!(parse_stream_title(block).as_deref(), Some("Artist - Song"));
    }

    #[test]
    fn empty_title() {
        let block = b"StreamTitle='';\0";
        assert_eq!(parse_stream_title(block).as_deref(), Some(""));
    }

    #[test]
    fn strips_metadata_from_stream() {
        // metaint = 4: 4 audio bytes, then an ICY metadata section, then audio.
        let mut raw = Vec::new();
        raw.extend_from_slice(b"AUDI");
        let payload = b"StreamTitle='Hi';";
        let blocks = payload.len().div_ceil(16); // 16-byte blocks needed
        let mut meta = payload.to_vec();
        meta.resize(blocks * 16, 0);
        raw.push(blocks as u8); // length prefix, in 16-byte units
        raw.extend_from_slice(&meta);
        raw.extend_from_slice(b"OOO");

        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let seen2 = seen.clone();
        let mut reader = IcyReader::new(
            io::Cursor::new(raw),
            4,
            Box::new(move |t| *seen2.lock().unwrap() = t),
        );
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"AUDIOOO");
        assert_eq!(seen.lock().unwrap().as_deref(), Some("Hi"));
    }
}
