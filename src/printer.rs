use std::io::Write;

use ansi_term::Colour::Fixed;
use ansi_term::Style;
use style::OutputComponents;
use syntax_mapping::SyntaxMapping;

use syntect::easy::HighlightLines;
use syntect::highlighting::{self, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use unicode_width::UnicodeWidthStr;

use content_inspector::ContentType;

use encoding::all::{UTF_16BE, UTF_16LE};
use encoding::{DecoderTrap, Encoding};

use assets::HighlightingAssets;
use errors::*;
use inputfile::{InputFile, InputFileReader};
use preprocessor::{expand_tabs, replace_nonprintable};
use style::OutputWrap;
use terminal::{as_terminal_escaped, to_ansi_color};

pub trait Printer {
    fn print_header(
        &mut self,
        handle: &mut Write,
        file: &InputFile,
        header_overwrite: Option<String>,
    ) -> Result<()>;
    fn print_footer(&mut self, handle: &mut Write) -> Result<()>;
    fn print_line(
        &mut self,
        out_of_range: bool,
        handle: &mut Write,
        line_number: usize,
        line_buffer: &[u8],
    ) -> Result<()>;
}

pub struct Frame {
    gutter: Option<&'static str>,
    term_width: usize,
    line_number_width: usize,
}

const LNUM_DIGITS: usize = 4;

impl Frame {
    fn new(term_width: usize, numbers: bool, grid: bool) -> Self {
        let separator = if grid { " │ " } else { " " };
        let term_width_needed = LNUM_DIGITS + separator.len() + 5;
        let (gutter, line_number_width) = if numbers && term_width >= term_width_needed {
            (Some(separator), LNUM_DIGITS)
        } else {
            (None, 0)
        };

        Frame {
            gutter,
            term_width,
            line_number_width,
        }
    }

    fn horizontal_line(&self, grid_char: char) -> String {
        fn hchars(n: usize) -> String {
            "─".repeat(n)
        }

        if self.line_number_width == 0 {
            hchars(self.term_width)
        } else {
            const GRID_CHAR_WIDTH: usize = 1;
            let prefix_width = self.line_number_width + 1; // Line number and a space character
            let suffix_width = self.term_width - prefix_width - GRID_CHAR_WIDTH;
            format!(
                "{}{}{}",
                hchars(prefix_width),
                grid_char,
                hchars(suffix_width)
            )
        }
    }

    fn numbered_gutter(&mut self, line_number: usize) -> Option<String> {
        self.gutter.map(|separator| {
            let n = format!("{:4}", line_number);
            self.line_number_width = n.len();
            n + separator
        })
    }

    fn blank_gutter(&self) -> Option<String> {
        self.gutter
            .map(|separator| " ".repeat(self.line_number_width) + separator)
    }
}

#[test]
fn large_line_numbers_modify_the_frame() {
    const CHECK: char = '✔';
    const CHAR_LEN: usize = 3; // UTF8 length of the "─" chars in `horizontal_line`'s output

    let mut frame = Frame::new(20, true, true);

    // Set normal_check to the position of the vertical-bar intersection point of a horizontal
    // line, before we print large numbers
    let header = frame.horizontal_line(CHECK);
    let header_check = header.find(CHECK).unwrap();

    // Normal number
    let small_number = frame.numbered_gutter(9999).unwrap();
    let small_blank = frame.blank_gutter().unwrap();
    assert_eq!(small_number.len(), small_blank.len());

    // Five-digit number
    let large_number = frame.numbered_gutter(10000).unwrap();
    let large_blank = frame.blank_gutter().unwrap();
    assert_eq!(large_number.len(), large_blank.len());
    assert_ne!(small_number.len(), large_number.len());

    // Set footer_check to the position of the vertical-bar intersection point of a horizontal
    // line after we print large numbers
    let footer = frame.horizontal_line(CHECK);
    let footer_check = footer.find(CHECK).unwrap();

    assert_eq!(header_check + CHAR_LEN, footer_check);

    let actual = format!(
        "*\n{}*\n{}*\n{}*\n{}*\n{}*\n{}*\n*",
        header, small_number, small_blank, large_number, large_blank, footer
    );
    let expected = "*
─────✔──────────────*
9999 │ *
     │ *
10000 │ *
      │ *
──────✔─────────────*
*";
    assert_eq!(actual, expected);
}

struct Colorize {
    colors: Colors,
    colored_output: bool,
    true_color: bool,
    use_italic_text: bool,
}

impl Colorize {
    fn filename<S: AsRef<str>>(&self, name: S) -> String {
        self.colors.filename.paint(name.as_ref()).to_string()
    }

    fn gutter<S: AsRef<str>>(&self, gutter_text: S) -> String {
        self.colors.grid.paint(gutter_text.as_ref()).to_string()
    }

    fn region<S: AsRef<str>>(&self, style: highlighting::Style, text: S) -> String {
        as_terminal_escaped(
            style,
            text.as_ref(),
            self.true_color,
            self.colored_output,
            self.use_italic_text,
        )
    }
}

pub struct InteractivePrinter<'a> {
    colorize: Colorize,
    frame: Frame,
    content_type: ContentType,
    highlighter: Option<HighlightLines<'a>>,
    syntax_set: &'a SyntaxSet,
    output_components: OutputComponents,
    term_width: usize,
    tab_width: usize,
    show_nonprintable: bool,
    output_wrap: OutputWrap,
}

impl<'a> InteractivePrinter<'a> {
    pub fn new(
        assets: &'a HighlightingAssets,
        file: &InputFile,
        reader: &mut InputFileReader,
        output_components: OutputComponents,
        theme: String,
        colored_output: bool,
        true_color: bool,
        term_width: usize,
        language: Option<String>,
        syntax_mapping: SyntaxMapping,
        tab_width: usize,
        show_nonprintable: bool,
        output_wrap: OutputWrap,
        use_italic_text: bool,
    ) -> Self {
        let theme = assets.get_theme(&theme);
        let syntax = assets.get_syntax(language, file, reader, &syntax_mapping);
        let syntax_set = &assets.syntax_set;
        InteractivePrinter::new2(
            theme,
            syntax,
            syntax_set,
            reader.content_type,
            output_components,
            colored_output,
            true_color,
            term_width,
            tab_width,
            show_nonprintable,
            output_wrap,
            use_italic_text,
        )
    }

    pub(crate) fn new2(
        theme: &'a Theme,
        syntax: &'a SyntaxReference,
        syntax_set: &'a SyntaxSet,
        content_type: ContentType,
        output_components: OutputComponents,
        colored_output: bool,
        true_color: bool,
        term_width: usize,
        tab_width: usize,
        show_nonprintable: bool,
        output_wrap: OutputWrap,
        use_italic_text: bool,
    ) -> Self {
        let colorize = Colorize{
            colors: if colored_output {
                Colors::colored(theme, true_color)
            } else {
                Colors::plain()
            },
            colored_output,
            true_color,
            use_italic_text,
        };

        let frame = Frame::new(
            term_width,
            output_components.numbers(),
            output_components.grid(),
        );

        let highlighter = if content_type.is_binary() {
            None
        } else {
            // Determine the type of syntax for highlighting
            Some(HighlightLines::new(syntax, theme))
        };

        InteractivePrinter {
            frame,
            colorize,
            content_type,
            highlighter,
            syntax_set,
            output_components,
            term_width,
            tab_width,
            show_nonprintable,
            output_wrap,
        }
    }

    fn print_horizontal_line(&mut self, handle: &mut Write, grid_char: char) -> Result<()> {
        writeln!(
            handle,
            "{}",
            self.colorize.gutter(self.frame.horizontal_line(grid_char))
        )?;
        Ok(())
    }

    fn preprocess(&self, text: &str, cursor: &mut usize) -> String {
        if self.tab_width > 0 {
            expand_tabs(text, self.tab_width, cursor)
        } else {
            text.to_string()
        }
    }

}

impl<'a> Printer for InteractivePrinter<'a> {
    fn print_header(
        &mut self,
        handle: &mut Write,
        file: &InputFile,
        header_overwrite: Option<String>,
    ) -> Result<()> {
        if !self.output_components.header() {
            return Ok(());
        }

        if self.output_components.grid() {
            self.print_horizontal_line(handle, '┬')?;
        };

        if let Some(gutter_text) = self.frame.blank_gutter() {
            write!(handle, "{}", self.colorize.gutter(gutter_text))?;
        };

        let (prefix, name): (&str, String) = match header_overwrite {
            Some(overwrite) => ("", overwrite),
            None => match file {
                InputFile::Ordinary(filename) => ("File: ", filename.to_string()),
                InputFile::String(_) => ("", "".to_string()),
                // _ => ("", &"STDIN".to_string()),
                _ => unimplemented!(),
            },
        };

        let mode = match self.content_type {
            ContentType::BINARY => "   <BINARY>",
            ContentType::UTF_16LE => "   <UTF-16LE>",
            ContentType::UTF_16BE => "   <UTF-16BE>",
            _ => "",
        };

        writeln!(handle, "{}{}{}", prefix, self.colorize.filename(&name), mode)?;

        if self.output_components.grid() {
            if self.content_type.is_text() {
                self.print_horizontal_line(handle, '┼')?;
            } else {
                self.print_horizontal_line(handle, '┴')?;
            }
        }

        Ok(())
    }

    fn print_footer(&mut self, handle: &mut Write) -> Result<()> {
        if self.output_components.grid() && self.content_type.is_text() {
            self.print_horizontal_line(handle, '┴')
        } else {
            Ok(())
        }
    }

    fn print_line(
        &mut self,
        out_of_range: bool,
        handle: &mut Write,
        line_number: usize,
        line_buffer: &[u8],
    ) -> Result<()> {
        let mut line = match self.content_type {
            ContentType::BINARY => {
                return Ok(());
            }
            ContentType::UTF_16LE => UTF_16LE
                .decode(&line_buffer, DecoderTrap::Strict)
                .unwrap_or("Invalid UTF-16LE".into()),
            ContentType::UTF_16BE => UTF_16BE
                .decode(&line_buffer, DecoderTrap::Strict)
                .unwrap_or("Invalid UTF-16BE".into()),
            _ => String::from_utf8_lossy(&line_buffer).to_string(),
        };

        if self.show_nonprintable {
            line = replace_nonprintable(&mut line, self.tab_width);
        }

        let regions = if let Some(ref mut highlighter) = self.highlighter {
            highlighter.highlight(line.as_ref(), self.syntax_set)
        } else {
            return Ok(());
        };

        if out_of_range {
            return Ok(());
        }

        let mut cursor: usize = 0;
        let mut cursor_max: usize = self.term_width;
        let mut cursor_total: usize = 0;
        let mut panel_wrap = "".to_string();

        // Frame gutter
        if let Some(gutter_text) = self.frame.numbered_gutter(line_number) {
            write!(handle, "{}", self.colorize.gutter(&gutter_text))?;
            cursor_max -= UnicodeWidthStr::width(&gutter_text[..]);
        }

        // Line contents.
        for &(style, region) in regions.iter() {
            let text = self.preprocess(
                region.trim_right_matches(|c| c == '\r' || c == '\n'),
                &mut cursor_total,
            );

            if self.output_wrap == OutputWrap::None {
                write!(handle, "{}", self.colorize.region(style, text),)?;
            } else {
                let mut chars = text.chars();
                let mut remaining = text.chars().count();

                while remaining > 0 {
                    let available = cursor_max - cursor;

                    if remaining <= available {
                        // It fits.
                        let text = chars.by_ref().take(remaining).collect::<String>();
                        cursor += remaining;

                        write!(handle, "{}", self.colorize.region(style, text))?;
                        break;
                    }

                    // Generate wrap padding if not already generated.
                    if panel_wrap.is_empty() {
                        if let Some(gutter_text) = self.frame.blank_gutter() {
                            panel_wrap = self.colorize.gutter(&gutter_text)
                        }
                    }

                    // It wraps.
                    let text = chars.by_ref().take(available).collect::<String>();
                    cursor = 0;
                    remaining -= available;

                    write!(
                        handle,
                        "{}\n{}",
                        self.colorize.region(style, text),
                        &panel_wrap,
                    )?;
                }
            }
        }
        write!(handle, "\n")?;

        Ok(())
    }
}

const DEFAULT_GUTTER_COLOR: u8 = 238;

#[derive(Default)]
pub struct Colors {
    pub grid: Style,
    pub filename: Style,
}

impl Colors {
    fn plain() -> Self {
        Colors::default()
    }

    pub fn colored(theme: &Theme, true_color: bool) -> Self {
        let gutter_color = theme
            .settings
            .gutter_foreground
            .map(|c| to_ansi_color(c, true_color))
            .unwrap_or(Fixed(DEFAULT_GUTTER_COLOR));

        Colors {
            grid: gutter_color.normal(),
            filename: Style::new().bold(),
        }
    }
}
