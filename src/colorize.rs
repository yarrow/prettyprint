use ansi_term as ansi;
use syntect::highlighting as sublime;

#[derive(Clone, Copy)]
pub(crate) enum ColorizeTo {
    Plain,
    Html {
        gutter_color: sublime::Color,
    },
    Terminal {
        gutter_color: sublime::Color,
        true_color: bool,
        use_italic_text: bool,
    },
}

pub(crate) trait Colorize {
    fn filename(&self, name: &str) -> String;
    fn gutter(&self, gutter_text: &str) -> String;
    fn region(&self, style: sublime::Style, text: &str) -> String;
}

pub(crate) fn new_colorize(colorize_to: ColorizeTo) -> Box<dyn Colorize> {
    use self::ColorizeTo::*;
    match colorize_to {
        Plain => Box::new(ColorizeNone()),
        Html { .. } => unimplemented!(),
        Terminal {
            gutter_color,
            true_color,
            use_italic_text,
        } => Box::new(ColorizeANSI::new(gutter_color, true_color, use_italic_text)),
    }
}

// Two ways of specifying a particular shade of gray.
const SUBLIME_DEFAULT_GUTTER_COLOR: sublime::Color = sublime::Color {
    r: 68,
    g: 68,
    b: 68,
    a: 255,
};
const ANSI_DEFAULT_GUTTER_COLOR: ansi::Color = ansi::Color::Fixed(238_u8);

pub(crate) fn make_gutter_color(c: Option<sublime::Color>) -> sublime::Color {
    c.unwrap_or(SUBLIME_DEFAULT_GUTTER_COLOR)
}

fn to_ansi_color(color: sublime::Color, true_color: bool) -> ansi_term::Colour {
    // TODO: Remove the first arm once we no longer need to exactly agree with
    // the original ANSI coloring. Or else check for every fixed-color ANSI encoding.
    if color == SUBLIME_DEFAULT_GUTTER_COLOR {
        ANSI_DEFAULT_GUTTER_COLOR
    } else if true_color {
        ansi::Color::RGB(color.r, color.g, color.b)
    } else {
        ansi::Color::Fixed(ansi_colours::ansi256_from_rgb((color.r, color.g, color.b)))
    }
}

struct ColorizeANSI {
    colors: Colors,
    true_color: bool,
    use_italic_text: bool,
}

pub(crate) struct Colors {
    pub grid: ansi::Style,
    pub filename: ansi::Style,
}

impl ColorizeANSI {
    fn new(gutter_color: sublime::Color, true_color: bool, use_italic_text: bool) -> Self {
        let colors = Colors {
            grid: to_ansi_color(gutter_color, true_color).normal(),
            filename: ansi::Style::new().bold(),
        };
        Self {
            colors,
            true_color,
            use_italic_text,
        }
    }
}

impl Colorize for ColorizeANSI {
    fn filename(&self, name: &str) -> String {
        self.colors.filename.paint(name).to_string()
    }

    fn gutter(&self, gutter_text: &str) -> String {
        self.colors.grid.paint(gutter_text).to_string()
    }

    fn region(&self, style: sublime::Style, text: &str) -> String {
        let font_style = style.font_style;
        let ansi_style = ansi::Style {
            foreground: Some(to_ansi_color(style.foreground, self.true_color)),
            is_bold: font_style.contains(sublime::FontStyle::BOLD),
            is_underline: font_style.contains(sublime::FontStyle::UNDERLINE),
            is_italic: self.use_italic_text && font_style.contains(sublime::FontStyle::ITALIC),
            ..ansi::Style::default()
        };

        ansi_style.paint(text).to_string()
    }
}

struct ColorizeNone();
impl Colorize for ColorizeNone {
    fn filename(&self, name: &str) -> String {
        name.to_string()
    }

    fn gutter(&self, gutter_text: &str) -> String {
        gutter_text.to_string()
    }

    fn region(&self, _style: sublime::Style, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod test {
    use self::sublime::{Color, FontStyle, Style};
    use super::*;

    fn black_text() -> Style {
        Style {
            foreground: Color::BLACK,
            background: Color::WHITE,
            font_style: FontStyle::empty(),
        }
    }
    fn red_text() -> Style {
        const RED: Color = Color {
            r: 255,
            ..Color::BLACK
        };
        Style {
            foreground: RED,
            background: Color::WHITE,
            font_style: FontStyle::empty(),
        }
    }
    #[test]
    fn test_plain() {
        let colorize = ColorizeNone();
        let original = "abc\nefg\n";
        assert_eq!(colorize.filename(original), original);
        assert_eq!(colorize.gutter(original), original);
        assert_eq!(colorize.region(red_text(), original), original);
        assert_eq!(colorize.region(black_text(), original), original);
    }
    #[test]
    fn colorize_none_when_colored_output_is_false() {
        let colorize = new_colorize(ColorizeTo::Plain);
        let original = "abc\nefg\n";
        assert_eq!(colorize.region(red_text(), original), original);
    }

    // The following is inaccurate for ANSI codes where one of the red, green, or blue values is
    // 1, 3, or 4 — it will mistake those for bold, italic or underlines font styles respectively.
    fn font_style_of(text: &str) -> FontStyle {
        // CSI: Control Sequence Introducer — ESC [
        // SGR: Select graphic rendition: a series of integer literals followed by 'm'
        assert!(
            &text[0..2] == "\u{1b}[",
            "Text doesn't begin with ANS CSI: {:?}",
            text
        );
        let mut font_style = FontStyle::empty();
        let sgr = match text.find('m') {
            None => panic!("Didn't find end of SGR in text: {:?}", text),
            Some(n) => text[2..n].split(";").fuse(),
        };
        for p in sgr {
            match p {
                "1" => font_style.insert(FontStyle::BOLD),
                "3" => font_style.insert(FontStyle::ITALIC),
                "4" => font_style.insert(FontStyle::UNDERLINE),
                _ => {}
            }
        }
        font_style
    }

    fn terminal(true_color: bool, use_italic_text: bool) -> Box<dyn Colorize> {
        new_colorize(ColorizeTo::Terminal {
            gutter_color: SUBLIME_DEFAULT_GUTTER_COLOR,
            true_color,
            use_italic_text,
        })
    }

    #[test]
    fn colorize_ansi_uses_italic_font_style_only_when_use_italic_text_is_true() {
        let mut bold_italic = FontStyle::ITALIC;
        bold_italic.insert(FontStyle::BOLD);
        let text = "Text";
        let style = Style {
            font_style: bold_italic,
            ..Style::default()
        };
        let without_italic = terminal(false, false).region(style, text);
        let with_italic = terminal(false, true).region(style, text);
        assert_eq!(font_style_of(&without_italic), FontStyle::BOLD);
        assert_eq!(font_style_of(&with_italic), bold_italic);
    }

    #[test]
    fn colorize_ansi_uses_256_color_mode_when_true_color_is_false() {
        const RED_24K: &str = "38;2;255;0;0";
        const RED_256: &str = "38;5;196";
        const TEXT: &str = "Text";
        let c_24k = terminal(true, false).region(red_text(), TEXT);
        let c_256 = terminal(false, false).region(red_text(), TEXT);
        assert!(c_24k.contains(RED_24K));
        assert!(!c_24k.contains(RED_256));
        assert!(c_256.contains(RED_256));
        assert!(!c_256.contains(RED_24K));
    }
}
