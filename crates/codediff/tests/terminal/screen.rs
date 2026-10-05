const WIDTH: u16 = 100;
const HEIGHT: u16 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Colour {
    Ansi(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, Default)]
struct Cell {
    symbol: char,
    foreground: Option<Colour>,
    background: Option<Colour>,
}

pub(crate) struct Screen {
    width: usize,
    height: usize,
    cursor_x: usize,
    cursor_y: usize,
    foreground: Option<Colour>,
    background: Option<Colour>,
    cells: Vec<Cell>,
}

impl Screen {
    pub(crate) fn parse(output: &str) -> Self {
        let mut screen = Self {
            width: WIDTH as usize,
            height: HEIGHT as usize,
            cursor_x: 0,
            cursor_y: 0,
            foreground: None,
            background: None,
            cells: vec![Cell::default(); WIDTH as usize * HEIGHT as usize],
        };
        screen.feed(output.as_bytes());
        screen
    }

    fn feed(&mut self, bytes: &[u8]) {
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                0x1b => {
                    at = self.escape(bytes, at + 1);
                }
                b'\r' => {
                    self.cursor_x = 0;
                    at += 1;
                }
                b'\n' => {
                    self.cursor_y = (self.cursor_y + 1).min(self.height.saturating_sub(1));
                    at += 1;
                }
                b'\x08' => {
                    self.cursor_x = self.cursor_x.saturating_sub(1);
                    at += 1;
                }
                b'\t' => {
                    self.cursor_x = (self.cursor_x + 8) / 8 * 8;
                    at += 1;
                }
                byte if byte.is_ascii_control() => at += 1,
                _ => {
                    let text = std::str::from_utf8(&bytes[at..]).unwrap_or("�");
                    let character = text.chars().next().unwrap_or('�');
                    self.put(character);
                    at += character.len_utf8();
                }
            }
        }
    }

    fn escape(&mut self, bytes: &[u8], mut at: usize) -> usize {
        let Some(&kind) = bytes.get(at) else {
            return at;
        };
        if kind == b'[' {
            at += 1;
            let start = at;
            while let Some(&byte) = bytes.get(at) {
                if (0x40..=0x7e).contains(&byte) {
                    self.csi(&bytes[start..at], byte as char);
                    return at + 1;
                }
                at += 1;
            }
            return at;
        }
        if kind == b']' {
            at += 1;
            while at < bytes.len() {
                if bytes[at] == 0x07 {
                    return at + 1;
                }
                if bytes[at] == 0x1b && bytes.get(at + 1) == Some(&b'\\') {
                    return at + 2;
                }
                at += 1;
            }
            return at;
        }
        at + 1
    }

    fn csi(&mut self, raw: &[u8], command: char) {
        let text = String::from_utf8_lossy(raw);
        let text = text.trim_start_matches(['?', '>']);
        let params: Vec<u16> = text
            .split(';')
            .map(|part| part.parse().unwrap_or(0))
            .collect();
        let first = |default| {
            params
                .first()
                .copied()
                .filter(|value| *value != 0)
                .unwrap_or(default)
        };
        match command {
            'H' | 'f' => {
                self.cursor_y =
                    usize::from(first(1).saturating_sub(1)).min(self.height.saturating_sub(1));
                self.cursor_x = usize::from(
                    params
                        .get(1)
                        .copied()
                        .filter(|value| *value != 0)
                        .unwrap_or(1)
                        .saturating_sub(1),
                )
                .min(self.width.saturating_sub(1));
            }
            'G' => {
                self.cursor_x =
                    usize::from(first(1).saturating_sub(1)).min(self.width.saturating_sub(1));
            }
            'd' => {
                self.cursor_y =
                    usize::from(first(1).saturating_sub(1)).min(self.height.saturating_sub(1));
            }
            'J' if first(0) == 2 || first(0) == 3 => {
                self.cells.fill(Cell::default());
            }
            'K' => {
                let y = self.cursor_y;
                for x in self.cursor_x..self.width {
                    self.cells[y * self.width + x] = Cell::default();
                }
            }
            'm' => self.sgr(&params),
            _ => {}
        }
    }

    fn sgr(&mut self, params: &[u16]) {
        let mut at = 0;
        while at < params.len() {
            match params[at] {
                0 => {
                    self.foreground = None;
                    self.background = None;
                }
                30..=37 => self.foreground = Some(Colour::Ansi((params[at] - 30) as u8)),
                90..=97 => self.foreground = Some(Colour::Ansi((params[at] - 90 + 8) as u8)),
                39 => self.foreground = None,
                40..=47 => self.background = Some(Colour::Ansi((params[at] - 40) as u8)),
                100..=107 => self.background = Some(Colour::Ansi((params[at] - 100 + 8) as u8)),
                49 => self.background = None,
                38 | 48 if params.get(at + 1) == Some(&5) && at + 2 < params.len() => {
                    let colour = Colour::Ansi(params[at + 2].min(255) as u8);
                    if params[at] == 38 {
                        self.foreground = Some(colour);
                    } else {
                        self.background = Some(colour);
                    }
                    at += 2;
                }
                38 | 48 if params.get(at + 1) == Some(&2) && at + 4 < params.len() => {
                    let colour = Colour::Rgb(
                        params[at + 2].min(255) as u8,
                        params[at + 3].min(255) as u8,
                        params[at + 4].min(255) as u8,
                    );
                    if params[at] == 38 {
                        self.foreground = Some(colour);
                    } else {
                        self.background = Some(colour);
                    }
                    at += 4;
                }
                _ => {}
            }
            at += 1;
        }
    }

    fn put(&mut self, symbol: char) {
        if self.cursor_x >= self.width || self.cursor_y >= self.height {
            return;
        }
        self.cells[self.cursor_y * self.width + self.cursor_x] = Cell {
            symbol,
            foreground: self.foreground,
            background: self.background,
        };
        self.cursor_x += 1;
    }

    fn line(&self, y: usize) -> String {
        (0..self.width)
            .map(|x| self.cells[y * self.width + x].symbol)
            .collect()
    }

    pub(crate) fn contains(&self, text: &str) -> bool {
        (0..self.height).any(|y| self.line(y).contains(text))
    }

    pub(crate) fn line_of(&self, text: &str) -> Option<usize> {
        (0..self.height).find(|&y| self.line(y).contains(text))
    }

    pub(crate) fn has_rgb_colour(&self) -> bool {
        self.cells.iter().any(|cell| {
            matches!(cell.foreground, Some(Colour::Rgb(..)))
                || matches!(cell.background, Some(Colour::Rgb(..)))
        })
    }

    pub(crate) fn vertical_bar_columns(&self) -> Vec<usize> {
        (0..self.width)
            .filter(|&x| {
                (0..self.height)
                    .filter(|&y| self.cells[y * self.width + x].symbol == '│')
                    .count()
                    >= 4
            })
            .collect()
    }

    pub(crate) fn styled_cells_in_line(&self, y: usize, from: usize) -> usize {
        (from..self.width)
            .filter(|&x| {
                let cell = self.cells[y * self.width + x];
                cell.foreground.is_some() || cell.background.is_some()
            })
            .count()
    }
}
