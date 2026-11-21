//! Bounded terminal query for background colour and 24-bit colour support.
//!
//! Sends OSC 11 (background), XTGETTCAP (`RGB` and `Tc`), and DA1 (`\x1b[c`)
//! as a single batch. Terminals reply in order, so the DA1 response marks the
//! end of the batch without waiting for the timeout.

use crate::theme::Rgb;

/// OSC 11 background query, XTGETTCAP (`RGB` = `524742`, `Tc` = `5463`), and DA1.
pub const QUERY: &[u8] = b"\x1b]11;?\x07\x1bP+q524742;5463\x1b\\\x1b[c";

/// What the terminal reported when queried directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProbe {
    pub background: Option<Rgb>,
    pub truecolor: Option<bool>,
}

/// Queries the attached terminal, waiting at most 40 ms for a reply.
pub fn probe() -> TerminalProbe {
    #[cfg(unix)]
    {
        unix::probe()
    }
    #[cfg(not(unix))]
    {
        TerminalProbe::default()
    }
}

/// Parses accumulated terminal replies for OSC 11 and XTGETTCAP.
pub(crate) fn parse_response(bytes: &[u8]) -> TerminalProbe {
    TerminalProbe {
        background: parse_osc11(bytes),
        truecolor: parse_xtgettcap(bytes),
    }
}

/// Whether `bytes` contains a complete Primary Device Attributes (`\x1b[?...c`) reply.
pub(crate) fn has_da1(bytes: &[u8]) -> bool {
    let mut at = 0;
    while at + 3 < bytes.len() {
        if bytes[at] == 0x1b && bytes[at + 1] == b'[' && bytes[at + 2] == b'?' {
            let mut cursor = at + 3;
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                if byte == b'c' {
                    return true;
                }
                if !(byte.is_ascii_digit() || byte == b';') {
                    break;
                }
                cursor += 1;
            }
        }
        at += 1;
    }
    false
}

/// Extracts an `Rgb` colour from `\x1b]11;rgb:RRRR/GGGG/BBBB(\x07|\x1b\\)`.
fn parse_osc11(bytes: &[u8]) -> Option<Rgb> {
    const PREFIX: &[u8] = b"\x1b]11;";
    let mut at = 0;
    while at + PREFIX.len() < bytes.len() {
        if bytes[at..].starts_with(PREFIX) {
            let start = at + PREFIX.len();
            if let Some((payload, next)) = terminated(&bytes[start..]) {
                if let Some(colour) = parse_rgb_spec(payload) {
                    return Some(colour);
                }
                at = start + next;
                continue;
            }
            return None;
        }
        at += 1;
    }
    None
}

/// Parses `XTGETTCAP` replies (`\x1bP1+r...\x1b\\` supported, `\x1bP0+r...\x1b\\` absent).
fn parse_xtgettcap(bytes: &[u8]) -> Option<bool> {
    let mut saw_negative = false;
    let mut at = 0;
    while at + 5 < bytes.len() {
        if bytes[at] == 0x1b
            && bytes[at + 1] == b'P'
            && bytes[at + 3] == b'+'
            && bytes[at + 4] == b'r'
        {
            let status = bytes[at + 2];
            let start = at + 5;
            let (_, next) = st_terminated(&bytes[start..])?;
            if status == b'1' {
                return Some(true);
            }
            if status == b'0' {
                saw_negative = true;
            }
            at = start + next;
            continue;
        }
        at += 1;
    }
    if saw_negative { Some(false) } else { None }
}

/// Finds a BEL (`\x07`) or ST (`\x1b\\`) terminator.
fn terminated(bytes: &[u8]) -> Option<(&[u8], usize)> {
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == 0x07 {
            return Some((&bytes[..at], at + 1));
        }
        if bytes[at] == 0x1b && bytes.get(at + 1) == Some(&b'\\') {
            return Some((&bytes[..at], at + 2));
        }
        at += 1;
    }
    None
}

/// Finds an ST (`\x1b\\`) terminator for DCS strings.
fn st_terminated(bytes: &[u8]) -> Option<(&[u8], usize)> {
    let mut at = 0;
    while at + 1 < bytes.len() {
        if bytes[at] == 0x1b && bytes[at + 1] == b'\\' {
            return Some((&bytes[..at], at + 2));
        }
        at += 1;
    }
    None
}

/// Parses `rgb:R/G/B` with 1..=4 hex digits per channel (XParseColor).
fn parse_rgb_spec(payload: &[u8]) -> Option<Rgb> {
    let text = std::str::from_utf8(payload).ok()?;
    let channels = text.strip_prefix("rgb:")?;
    let mut parts = channels.split('/');
    let r = parse_channel(parts.next()?)?;
    let g = parse_channel(parts.next()?)?;
    let b = parse_channel(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(Rgb(r, g, b))
}

fn parse_channel(hex: &str) -> Option<u8> {
    let digits = hex.len();
    if !(1..=4).contains(&digits) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    let max = (1u32 << (digits * 4)) - 1;
    Some(((value * 255 + max / 2) / max) as u8)
}

#[cfg(unix)]
mod unix {
    use std::io::{IsTerminal, Write};
    use std::time::{Duration, Instant};

    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::io::Errno;
    use rustix::termios::{OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr};

    use super::{QUERY, TerminalProbe, has_da1, parse_response};

    const TIMEOUT: Duration = Duration::from_millis(40);

    struct RawGuard {
        original: Termios,
    }

    impl RawGuard {
        fn enter() -> Option<Self> {
            let stdin = rustix::stdio::stdin();
            let original = tcgetattr(stdin).ok()?;
            let mut raw = original.clone();
            raw.make_raw();
            raw.special_codes[SpecialCodeIndex::VMIN] = 0;
            raw.special_codes[SpecialCodeIndex::VTIME] = 0;
            tcsetattr(stdin, OptionalActions::Now, &raw).ok()?;
            Some(Self { original })
        }
    }

    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = tcsetattr(rustix::stdio::stdin(), OptionalActions::Now, &self.original);
        }
    }

    pub(super) fn probe() -> TerminalProbe {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return TerminalProbe::default();
        }
        let Some(_guard) = RawGuard::enter() else {
            return TerminalProbe::default();
        };

        // Discard unread bytes sitting on stdin so we only inspect the reply.
        drain_pending();

        {
            let mut stdout = std::io::stdout().lock();
            if stdout.write_all(QUERY).is_err() || stdout.flush().is_err() {
                return TerminalProbe::default();
            }
        }

        let stdin = rustix::stdio::stdin();
        let deadline = Instant::now() + TIMEOUT;
        let mut reply = Vec::with_capacity(128);
        let mut chunk = [0u8; 128];

        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            if remaining.is_zero() {
                break;
            }
            let timeout = Timespec {
                tv_sec: remaining.as_secs() as _,
                tv_nsec: remaining.subsec_nanos() as _,
            };
            let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(0) => break,
                Ok(_) => loop {
                    match rustix::io::read(stdin, &mut chunk) {
                        Ok(0) => break,
                        Ok(read) => {
                            reply.extend_from_slice(&chunk[..read]);
                            if reply.len() >= 1024 {
                                return parse_response(&reply);
                            }
                        }
                        Err(Errno::INTR) => continue,
                        Err(_) => break,
                    }
                },
                Err(Errno::INTR) => continue,
                Err(_) => break,
            }
            if has_da1(&reply) {
                break;
            }
        }

        parse_response(&reply)
    }

    fn drain_pending() {
        let stdin = rustix::stdio::stdin();
        let zero = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut chunk = [0u8; 128];
        loop {
            let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
            match poll(&mut fds, Some(&zero)) {
                Ok(0) | Err(_) => break,
                Ok(_) => match rustix::io::read(stdin, &mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_four_digit_osc11_with_bel_and_st() {
        let latte = b"\x1b]11;rgb:eff1/f5f5/ffff\x07\x1b[?62;22c";
        let probed = parse_response(latte);
        assert_eq!(probed.background, Some(Rgb(0xef, 0xf5, 0xff)));
        assert!(!probed.background.unwrap().is_dark());
        assert!(has_da1(latte));

        let mocha = b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\\x1b[?1;2c";
        let probed = parse_response(mocha);
        assert_eq!(probed.background, Some(Rgb(0x1e, 0x1e, 0x2e)));
        assert!(probed.background.unwrap().is_dark());
        assert!(has_da1(mocha));
    }

    #[test]
    fn parses_one_two_and_three_digit_osc11_channels() {
        assert_eq!(
            parse_response(b"\x1b]11;rgb:1e/1e/2e\x07").background,
            Some(Rgb(0x1e, 0x1e, 0x2e))
        );
        assert_eq!(
            parse_response(b"\x1b]11;rgb:f/0/8\x07").background,
            Some(Rgb(255, 0, 136))
        );
        assert_eq!(
            parse_response(b"\x1b]11;rgb:fff/000/800\x1b\\").background,
            Some(Rgb(255, 0, 128))
        );
    }

    #[test]
    fn parses_xtgettcap_positive_and_negative_replies() {
        assert_eq!(
            parse_response(b"\x1bP1+r524742=382f382f38\x1b\\").truecolor,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP0+r524742\x1b\\\x1bP1+r5463\x1b\\").truecolor,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP0+r524742\x1b\\\x1bP0+r5463\x1b\\").truecolor,
            Some(false)
        );
        assert_eq!(parse_response(b"\x1b[?62c").truecolor, None);
    }

    #[test]
    fn chunked_bytes_wait_until_da1_completes() {
        let full = b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1bP1+r5463\x1b\\\x1b[?64;1;2;6;22c";
        let mut buffer = Vec::new();
        for &byte in &full[..full.len() - 1] {
            buffer.push(byte);
            assert!(!has_da1(&buffer));
        }
        buffer.push(*full.last().unwrap());
        assert!(has_da1(&buffer));
        assert_eq!(
            parse_response(&buffer),
            TerminalProbe {
                background: Some(Rgb(255, 255, 255)),
                truecolor: Some(true),
            }
        );
    }

    #[test]
    fn malformed_osc11_is_ignored_without_panicking() {
        for bad in [
            &b""[..],
            b"\x1b]11;rgb:zzzz/0000/0000\x07",
            b"\x1b]11;rgb:12345/0000/0000\x07",
            b"\x1b]11;rgb:ffff/ffff\x07",
            b"\x1b]11;rgb:ffff/ffff/ffff/ffff\x07",
            b"\x1b]11;rgb:ffff/ffff/ffff",
        ] {
            assert_eq!(parse_response(bad).background, None, "{bad:?}");
        }
    }
}
