//! Bounded terminal query for background colour, 24-bit colour, and graph symbols.
//!
//! Sends OSC 11 (background), XTGETTCAP (`RGB` and `Tc`), XTVERSION (`\x1b[>0q`),
//! and DA1 (`\x1b[c`) as a single batch. Terminals reply in order, so the DA1
//! response marks the end of the batch without waiting for the timeout.

use std::sync::OnceLock;

use crate::theme::Rgb;

static PROBED: OnceLock<TerminalProbe> = OnceLock::new();

/// OSC 11 background query, XTGETTCAP (`RGB` = `524742`, `Tc` = `5463`), XTVERSION, and DA1.
pub const QUERY: &[u8] = b"\x1b]11;?\x07\x1bP+q524742;5463\x1b\\\x1b[>0q\x1b[c";

/// What the terminal reported when queried directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProbe {
    pub background: Option<Rgb>,
    pub truecolor: Option<bool>,
    /// `Some(true)` or `Some(false)` when `XTVERSION` identified the terminal;
    /// `None` when the terminal sent no `XTVERSION` reply.
    pub branch_drawing: Option<bool>,
}

impl TerminalProbe {
    /// Resolves whether the terminal natively draws `U+F5D0..U+F60D` branch glyphs,
    /// combining a direct `XTVERSION` reply with environment fallbacks.
    pub fn supports_branch_drawing(self, environment: impl Fn(&str) -> Option<String>) -> bool {
        if let Some(supported) = self.branch_drawing {
            return supported;
        }
        if let Some(term) = environment("TERM")
            && (term.eq_ignore_ascii_case("xterm-kitty")
                || term.eq_ignore_ascii_case("xterm-ghostty")
                || term.eq_ignore_ascii_case("ghostty"))
        {
            return true;
        }
        if let Some(program) = environment("TERM_PROGRAM")
            && (program.eq_ignore_ascii_case("ghostty")
                || program.eq_ignore_ascii_case("vscode")
                || program.eq_ignore_ascii_case("kitty"))
        {
            return true;
        }
        false
    }
}

/// Queries the attached terminal once, waiting at most 40 ms for a reply, and caches the result.
pub fn probe() -> TerminalProbe {
    *PROBED.get_or_init(|| {
        #[cfg(unix)]
        {
            unix::probe()
        }
        #[cfg(not(unix))]
        {
            TerminalProbe::default()
        }
    })
}

/// Returns the cached startup [`TerminalProbe`] without touching the TTY.
pub fn cached_probe() -> TerminalProbe {
    PROBED.get().copied().unwrap_or_default()
}

/// Resolves whether branch-drawing symbols are supported from the cached probe and environment.
pub fn cached_branch_drawing() -> bool {
    cached_probe().supports_branch_drawing(|key| std::env::var(key).ok())
}

#[cfg(any(unix, test))]
mod parser {
    use super::TerminalProbe;
    use crate::theme::Rgb;

    /// Parses accumulated terminal replies for OSC 11, XTGETTCAP, and XTVERSION.
    pub(super) fn parse_response(bytes: &[u8]) -> TerminalProbe {
        TerminalProbe {
            background: parse_osc11(bytes),
            truecolor: parse_xtgettcap(bytes),
            branch_drawing: parse_xtversion(bytes),
        }
    }

    /// Whether `bytes` contains a complete Primary Device Attributes (`\x1b[?...c`) reply.
    pub(super) fn has_da1(bytes: &[u8]) -> bool {
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

    /// Parses `XTVERSION` DCS replies (`\x1bP>|<ident>\x1b\\`) and checks whether
    /// the identified terminal version natively draws `U+F5D0..U+F60D`.
    fn parse_xtversion(bytes: &[u8]) -> Option<bool> {
        const PREFIX: &[u8] = b"\x1bP>|";
        let mut at = 0;
        while at + PREFIX.len() < bytes.len() {
            if bytes[at..].starts_with(PREFIX) {
                let start = at + PREFIX.len();
                let (payload, _) = st_terminated(&bytes[start..])?;
                let ident = std::str::from_utf8(payload).ok()?.trim();
                return Some(xtversion_draws_branch_symbols(ident));
            }
            at += 1;
        }
        None
    }

    fn xtversion_draws_branch_symbols(ident: &str) -> bool {
        let (name, version) = split_terminal_ident(ident);
        let lower = name.to_ascii_lowercase();
        let numbers = version_numbers(version);
        match lower.as_str() {
            "xterm.js" => version_at_least(&numbers, &[5, 5, 0]),
            "kitty" => version_at_least(&numbers, &[0, 36, 2]),
            "ghostty" => version_at_least(&numbers, &[1, 0, 0]),
            "contour" => version_at_least(&numbers, &[0, 5, 0]),
            "wezterm" => {
                let date = version
                    .bytes()
                    .take_while(|byte| byte.is_ascii_digit())
                    .take(8)
                    .fold(0u32, |acc, byte| acc * 10 + u32::from(byte - b'0'));
                date >= 2024_0900
            }
            _ => false,
        }
    }

    fn split_terminal_ident(ident: &str) -> (&str, &str) {
        if let Some((name, rest)) = ident.split_once('(') {
            let version = rest.strip_suffix(')').unwrap_or(rest);
            return (name.trim(), version.trim());
        }
        if let Some((name, version)) = ident.split_once(' ') {
            return (name.trim(), version.trim());
        }
        (ident.trim(), "")
    }

    fn version_numbers(version: &str) -> Vec<u32> {
        let mut numbers = Vec::new();
        for part in version.split('.') {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            let Ok(number) = digits.parse::<u32>() else {
                break;
            };
            numbers.push(number);
        }
        numbers
    }

    fn version_at_least(actual: &[u32], minimum: &[u32]) -> bool {
        if actual.is_empty() {
            return false;
        }
        for (index, &min) in minimum.iter().enumerate() {
            let got = actual.get(index).copied().unwrap_or(0);
            if got != min {
                return got > min;
            }
        }
        true
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
}

#[cfg(unix)]
mod unix {
    use std::io::{IsTerminal, Write};
    use std::time::{Duration, Instant};

    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::io::Errno;
    use rustix::termios::{OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr};

    use super::parser::{has_da1, parse_response};
    use super::{QUERY, TerminalProbe};

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
    use super::parser::{has_da1, parse_response};
    use super::*;

    fn environment(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.to_owned())
        }
    }

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
    fn parses_xtversion_for_branch_drawing_support() {
        assert_eq!(
            parse_response(b"\x1bP>|xterm.js(6.1.0-beta.303)\x1b\\\x1b[?1;2c").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|xterm.js(5.5.0)\x1b\\").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|xterm.js(5.4.1)\x1b\\").branch_drawing,
            Some(false)
        );
        assert_eq!(
            parse_response(b"\x1bP>|kitty(0.38.0)\x1b\\").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|kitty(0.35.0)\x1b\\").branch_drawing,
            Some(false)
        );
        assert_eq!(
            parse_response(b"\x1bP>|ghostty 1.1.0\x1b\\").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|ghostty(0.9.0)\x1b\\").branch_drawing,
            Some(false)
        );
        assert_eq!(
            parse_response(b"\x1bP>|contour 0.5.1\x1b\\").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|WezTerm 20250203-110809\x1b\\").branch_drawing,
            Some(true)
        );
        assert_eq!(
            parse_response(b"\x1bP>|WezTerm 20240203-110809\x1b\\").branch_drawing,
            Some(false)
        );
        assert_eq!(
            parse_response(b"\x1bP>|Alacritty(0.15.0)\x1b\\").branch_drawing,
            Some(false)
        );
        assert_eq!(parse_response(b"\x1b[?62c").branch_drawing, None);
    }

    #[test]
    fn supports_branch_drawing_falls_back_to_environment_only_when_unprobed() {
        let unprobed = TerminalProbe::default();
        assert!(!unprobed.supports_branch_drawing(environment(&[])));
        assert!(unprobed.supports_branch_drawing(environment(&[("TERM", "xterm-kitty")])));
        assert!(unprobed.supports_branch_drawing(environment(&[("TERM", "xterm-ghostty")])));
        assert!(unprobed.supports_branch_drawing(environment(&[("TERM_PROGRAM", "vscode")])));
        assert!(!unprobed.supports_branch_drawing(environment(&[("TERM_PROGRAM", "Alacritty")])));

        let old_kitty = parse_response(b"\x1bP>|kitty(0.35.0)\x1b\\");
        assert!(!old_kitty.supports_branch_drawing(environment(&[("TERM", "xterm-kitty")])));
    }

    #[test]
    fn chunked_bytes_wait_until_da1_completes() {
        let full =
            b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1bP1+r5463\x1b\\\x1bP>|xterm.js(6.1.0)\x1b\\\x1b[?64;1;2;6;22c";
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
                branch_drawing: Some(true),
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
