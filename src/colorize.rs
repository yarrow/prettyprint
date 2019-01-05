use ansi_term as ansi;
use syntect::highlighting::{self, FontStyle, Theme};

pub(crate) trait Colorize {
    fn filename(&self, name: &str) -> String;
    fn gutter(&self, gutter_text: &str) -> String;
    fn region(&self, style: highlighting::Style, text: &str) -> String;
}

pub(crate) fn new_colorize(
    theme: &Theme,
    colored_output: bool,
    true_color: bool,
    use_italic_text: bool,
) -> Box<dyn Colorize> {
    match colored_output {
        false => Box::new(ColorizeNone()),
        true => Box::new(ColorizeANSI::new(theme, true_color, use_italic_text)),
    }
}

struct ColorizeANSI {
    colors: Colors,
    true_color: bool,
    use_italic_text: bool,
}

const DEFAULT_GUTTER_COLOR: u8 = 238;

pub(crate) struct Colors {
    pub grid: ansi::Style,
    pub filename: ansi::Style,
}

fn to_ansi_color(color: highlighting::Color, true_color: bool) -> ansi_term::Colour {
    if true_color {
        ansi::Color::RGB(color.r, color.g, color.b)
    } else {
        ansi::Color::Fixed(ansi_colours::ansi256_from_rgb((color.r, color.g, color.b)))
    }
}

impl ColorizeANSI {
    fn new(theme: &Theme, true_color: bool, use_italic_text: bool) -> Self {
        let gutter_color = theme
            .settings
            .gutter_foreground
            .map(|c| to_ansi_color(c, true_color))
            .unwrap_or(ansi::Color::Fixed(DEFAULT_GUTTER_COLOR));
        let colors = Colors {
            grid: gutter_color.normal(),
            filename: ansi::Style::new().bold(),
        };
        ColorizeANSI {
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

    fn region(&self, style: highlighting::Style, text: &str) -> String {
        let font_style = style.font_style;
        let ansi_style = ansi::Style {
            foreground: Some(to_ansi_color(style.foreground, self.true_color)),
            is_bold: font_style.contains(FontStyle::BOLD),
            is_underline: font_style.contains(FontStyle::UNDERLINE),
            is_italic: self.use_italic_text && font_style.contains(FontStyle::ITALIC),
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

    fn region(&self, _style: highlighting::Style, text: &str) -> String {
        text.to_string()
    }
}
