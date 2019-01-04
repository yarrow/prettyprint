extern crate ansi_colours;

use ansi_term::Colour::{Fixed, RGB};
use ansi_term::{self, Style};

use syntect::highlighting::{self, FontStyle};

pub fn to_ansi_color(color: highlighting::Color, true_color: bool) -> ansi_term::Colour {
    if true_color {
        RGB(color.r, color.g, color.b)
    } else {
        Fixed(ansi_colours::ansi256_from_rgb((color.r, color.g, color.b)))
    }
}

pub fn as_terminal_escaped(
    style: highlighting::Style,
    text: &str,
    true_color: bool,
    italics: bool,
) -> String {
    let font_style = style.font_style;
    let ansi_style = Style {
        foreground: Some(to_ansi_color(style.foreground, true_color)),
        is_bold: font_style.contains(FontStyle::BOLD),
        is_underline: font_style.contains(FontStyle::UNDERLINE),
        is_italic: italics && font_style.contains(FontStyle::ITALIC),
        ..Style::default()
    };

    ansi_style.paint(text).to_string()
}
