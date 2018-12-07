use ansi_term::Style;
use printer::Colors;

#[derive(Clone)]
pub struct DecorationText {
    pub width: usize,
    pub text: String,
}

pub trait Decoration {
    fn generate(&self, line_number: usize, continuation: bool) -> DecorationText;
    fn width(&self) -> usize;
}

pub struct LineNumberDecoration {
    color: Style,
    cached_wrap: DecorationText,
    cached_wrap_invalid_at: usize,
}

impl LineNumberDecoration {
    pub fn new(colors: &Colors) -> Self {
        LineNumberDecoration {
            color: colors.line_number,
            cached_wrap_invalid_at: 10000,
            cached_wrap: DecorationText {
                text: colors.line_number.paint(" ".repeat(4)).to_string(),
                width: 4,
            },
        }
    }
}

impl Decoration for LineNumberDecoration {
    fn generate(&self, line_number: usize, continuation: bool) -> DecorationText {
        if continuation {
            if line_number > self.cached_wrap_invalid_at {
                let new_width = self.cached_wrap.width + 1;
                return DecorationText {
                    text: self.color.paint(" ".repeat(new_width)).to_string(),
                    width: new_width,
                };
            }

            self.cached_wrap.clone()
        } else {
            let plain: String = format!("{:4}", line_number);
            DecorationText {
                width: plain.len(),
                text: self.color.paint(plain).to_string(),
            }
        }
    }

    fn width(&self) -> usize {
        4
    }
}

pub struct GridBorderDecoration {
    cached: DecorationText,
}

impl GridBorderDecoration {
    pub fn new(colors: &Colors) -> Self {
        GridBorderDecoration {
            cached: DecorationText {
                text: colors.grid.paint("│").to_string(),
                width: 1,
            },
        }
    }
}

impl Decoration for GridBorderDecoration {
    fn generate(&self, _line_number: usize, _continuation: bool) -> DecorationText {
        self.cached.clone()
    }

    fn width(&self) -> usize {
        self.cached.width
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assets::HighlightingAssets;
    use printer::Colors;
    use syntect::highlighting;

    fn default_gutter_theme() -> highlighting::Theme {
        const TEST_THEME: &str = "Monokai Extended"; // prettyprint's default at the time of writing
        let assets = HighlightingAssets::new();
        let mut theme = assets.get_theme(TEST_THEME).clone();
        theme.settings.gutter_foreground = None;
        theme
    }

    fn magenta_gutter_theme() -> highlighting::Theme {
        let mut theme = default_gutter_theme();
        theme.settings.gutter_foreground = Some(highlighting::Color {
            r: 255,
            g: 0,
            b: 255,
            a: 0,
        });
        theme
    }

    #[test]
    fn line_number_decorator_returns_line_number_or_blanks_colored_by_theme() {
        let colors = Colors::colored(&default_gutter_theme(), true);
        let decorator = LineNumberDecoration::new(&colors);
        assert_eq!(decorator.width(), 4);
        struct TestCase {
            line: usize,
            is_continuation: bool,
            expected: &'static str, // expected string, before ANSI colors
        }
        let tests = [
            TestCase {
                line: 0,
                is_continuation: false,
                expected: "   0",
            },
            TestCase {
                line: 9999,
                is_continuation: false,
                expected: "9999",
            },
            TestCase {
                line: 9999,
                is_continuation: true,
                expected: "    ",
            },
            TestCase {
                line: 10000,
                is_continuation: false,
                expected: "10000",
            },
        ];
        for t in tests.iter() {
            let line_number = decorator.generate(t.line, t.is_continuation);
            assert_eq!(
                line_number.text,
                format!("\u{1b}[38;5;238m{}\u{1b}[0m", t.expected)
            );
        }
    }

    #[test]
    fn grid_border_decorator_returns_vertical_bar_colored_by_theme() {
        let colors = Colors::colored(&magenta_gutter_theme(), true);
        let decorator = GridBorderDecoration::new(&colors);
        assert_eq!(decorator.width(), 1);
        let normal = decorator.generate(9999, false).text;
        let continuation = decorator.generate(0, true).text;
        assert_eq!(normal, continuation);
        assert_eq!(continuation, "\u{1b}[38;2;255;0;255m│\u{1b}[0m");
    }

    #[test]
    fn missing_gutter_foreground_makes_gray_decorations() {
        let colors = Colors::colored(&default_gutter_theme(), true);
        let decorator = GridBorderDecoration::new(&colors);
        assert_eq!(decorator.width(), 1);
        let normal = decorator.generate(9999, false).text;
        let continuation = decorator.generate(0, true).text;
        assert_eq!(normal, continuation);
        assert_eq!(
            continuation.as_bytes(),
            "\u{1b}[38;5;238m│\u{1b}[0m".as_bytes()
        );
    }

}
