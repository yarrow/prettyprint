use std::io::Write;

use ansi_term::Colour::Fixed;
use ansi_term::Style;
use style::OutputComponents;
use syntax_mapping::SyntaxMapping;

use console::AnsiCodeIterator;

use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};

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

pub struct InteractivePrinter<'a> {
    colors: Colors,
    decorations: Option<&'static str>,
    panel_width: usize,
    ansi_prefix_sgr: String,
    content_type: ContentType,
    highlighter: Option<HighlightLines<'a>>,
    syntax_set: &'a SyntaxSet,
    output_components: OutputComponents,
    colored_output: bool,
    true_color: bool,
    term_width: usize,
    tab_width: usize,
    show_nonprintable: bool,
    output_wrap: OutputWrap,
    use_italic_text: bool,
}

const LNUM_DIGITS: usize = 4;
fn lnum(line_number: usize, continuation: bool) -> String {
    let num = format!("{:4}", line_number);
    if continuation {
        " ".repeat(num.len())
    } else {
        num
    }
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
        let colors = if colored_output {
            Colors::colored(theme, true_color)
        } else {
            Colors::plain()
        };

        // Since the print_horizontal_line, print_header, and print_footer
        // functions all assume the panel width is without the grid border,
        // panel_width only counts the space for line numbers.
        let nominal_panel_width = LNUM_DIGITS + 1;
        let grid_str = if output_components.grid() { " │" } else { "" };

        let term_width_needed = nominal_panel_width + grid_str.len() + 5;
        let (decorations, panel_width) =
            if output_components.numbers() && term_width >= term_width_needed {
                (Some(grid_str), nominal_panel_width)
            } else {
                (None, 0)
            };

        let highlighter = if content_type.is_binary() {
            None
        } else {
            // Determine the type of syntax for highlighting
            Some(HighlightLines::new(syntax, theme))
        };

        InteractivePrinter {
            panel_width,
            colors,
            decorations,
            content_type,
            ansi_prefix_sgr: String::new(),
            highlighter,
            syntax_set,
            output_components,
            colored_output,
            true_color,
            term_width,
            tab_width,
            show_nonprintable,
            output_wrap,
            use_italic_text,
        }
    }

    fn print_horizontal_line(&mut self, handle: &mut Write, grid_char: char) -> Result<()> {
        if self.panel_width == 0 {
            writeln!(
                handle,
                "{}",
                self.color_gutter("─".repeat(self.term_width))
            )?;
        } else {
            let hline = "─".repeat(self.term_width - (self.panel_width + 1));
            let hline = format!("{}{}{}", "─".repeat(self.panel_width), grid_char, hline);
            writeln!(handle, "{}", self.color_gutter(hline))?;
        }

        Ok(())
    }

    fn preprocess(&self, text: &str, cursor: &mut usize) -> String {
        if self.tab_width > 0 {
            expand_tabs(text, self.tab_width, cursor)
        } else {
            text.to_string()
        }
    }

    fn color_filename<S: AsRef<str>>(&self, name: S) -> String {
        self.colors.filename.paint(name.as_ref()).to_string()
    }

    fn color_gutter<S: AsRef<str>>(&self, gutter_text: S) -> String {
        self.colors.grid.paint(gutter_text.as_ref()).to_string()
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

            write!(
                handle,
                "{}{}",
                " ".repeat(self.panel_width),
                self.color_gutter(if self.panel_width > 0 { "│ " } else { "" }),
            )?;
        } else {
            write!(handle, "{}", " ".repeat(self.panel_width))?;
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

        writeln!(handle, "{}{}{}", prefix, self.color_filename(&name), mode)?;

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
        let mut panel_wrap: Option<String> = None;

        // Line decorations.
        if let Some(grid_str) = self.decorations {
            let deco = lnum(line_number, false) + grid_str;
            write!(handle, "{} ", self.color_gutter(&deco))?;
            cursor_max -= deco.len() + 1;
        }

        // Line contents.
        if self.output_wrap == OutputWrap::None {
            let true_color = self.true_color;
            let colored_output = self.colored_output;
            let italics = self.use_italic_text;

            for &(style, region) in regions.iter() {
                let text = self.preprocess(region, &mut cursor_total);
                write!(
                    handle,
                    "{}",
                    as_terminal_escaped(style, &*text, true_color, colored_output, italics,)
                )?;
            }

            if line.bytes().next_back() != Some(b'\n') {
                write!(handle, "\n")?;
            }
        } else {
            for &(style, region) in regions.iter() {
                let mut ansi_iterator = AnsiCodeIterator::new(region);
                let mut ansi_prefix: String = String::new();
                for chunk in ansi_iterator {
                    match chunk {
                        // ANSI escape passthrough.
                        (text, true) => {
                            if text.chars().last().map_or(false, |c| c == 'm') {
                                ansi_prefix.push_str(text);
                                if text == "\x1B[0m" {
                                    self.ansi_prefix_sgr = "\x1B[0m".to_owned();
                                } else {
                                    self.ansi_prefix_sgr.push_str(text);
                                }
                            } else {
                                ansi_prefix.push_str(text);
                            }
                        }

                        // Regular text.
                        (text, false) => {
                            let text = self.preprocess(
                                text.trim_right_matches(|c| c == '\r' || c == '\n'),
                                &mut cursor_total,
                            );

                            let mut chars = text.chars();
                            let mut remaining = text.chars().count();

                            while remaining > 0 {
                                let available = cursor_max - cursor;

                                // It fits.
                                if remaining <= available {
                                    let text = chars.by_ref().take(remaining).collect::<String>();
                                    cursor += remaining;

                                    write!(
                                        handle,
                                        "{}",
                                        as_terminal_escaped(
                                            style,
                                            &*format!(
                                                "{}{}{}",
                                                self.ansi_prefix_sgr, ansi_prefix, text
                                            ),
                                            self.true_color,
                                            self.colored_output,
                                            self.use_italic_text
                                        )
                                    )?;
                                    break;
                                }

                                // Generate wrap padding if not already generated.
                                if panel_wrap.is_none() {
                                    panel_wrap = if let Some(grid_str) = self.decorations {
                                        let deco = lnum(line_number, true) + grid_str;
                                        Some(format!("{} ", self.color_gutter(&deco)))
                                    } else {
                                        Some("".to_string())
                                    }
                                }

                                // It wraps.
                                let text = chars.by_ref().take(available).collect::<String>();
                                cursor = 0;
                                remaining -= available;

                                write!(
                                    handle,
                                    "{}\n{}",
                                    as_terminal_escaped(
                                        style,
                                        &*format!(
                                            "{}{}{}",
                                            self.ansi_prefix_sgr, ansi_prefix, text
                                        ),
                                        self.true_color,
                                        self.colored_output,
                                        self.use_italic_text
                                    ),
                                    panel_wrap.clone().unwrap()
                                )?;
                            }

                            // Clear the ANSI prefix buffer.
                            ansi_prefix.clear();
                        }
                    }
                }
            }

            write!(handle, "\n")?;
        }

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
