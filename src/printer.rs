use std::io::Write;

use style::OutputComponents;
use syntax_mapping::SyntaxMapping;

use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};

use content_inspector::ContentType;

use encoding::all::{UTF_16BE, UTF_16LE};
use encoding::{DecoderTrap, Encoding};

use assets::HighlightingAssets;
use colorize::{new_colorize, Colorize};
use errors::*;
use frame::Frame;
use inputfile::{InputFile, InputFileReader};
use preprocessor::{expand_tabs, replace_nonprintable};
use style::OutputWrap;

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
    colorize: Box<dyn Colorize>,
    frame: Frame,
    content_type: ContentType,
    highlighter: Option<HighlightLines<'a>>,
    syntax_set: &'a SyntaxSet,
    output_components: OutputComponents,
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
        let theme_gutter = theme.settings.gutter_foreground;
        let colorize = new_colorize(theme_gutter, colored_output, true_color, use_italic_text);

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
            tab_width,
            show_nonprintable,
            output_wrap,
        }
    }

    fn print_horizontal_line(&mut self, handle: &mut Write, grid_char: char) -> Result<()> {
        writeln!(
            handle,
            "{}",
            self.colorize.gutter(&self.frame.horizontal_line(grid_char))
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
            write!(handle, "{}", self.colorize.gutter(&gutter_text))?;
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

        writeln!(
            handle,
            "{}{}{}",
            prefix,
            self.colorize.filename(&name),
            mode
        )?;

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

        let cursor_max: usize = self.frame.cursor_max();
        let mut cursor: usize = 0;
        let mut cursor_total: usize = 0;
        let mut panel_wrap = "".to_string();

        // Frame gutter
        if let Some(gutter_text) = self.frame.numbered_gutter(line_number) {
            write!(handle, "{}", self.colorize.gutter(&gutter_text))?;
        }

        // Line contents.
        if self.output_wrap == OutputWrap::None {
            for &(style, region) in regions.iter() {
                let text = self.preprocess(region, &mut cursor_total);
                write!(handle, "{}", self.colorize.region(style, &text),)?;
            }

            if line.bytes().next_back() != Some(b'\n') {
                write!(handle, "\n")?;
            }
        } else {
            for &(style, region) in regions.iter() {
                let text = self.preprocess(
                    region.trim_right_matches(|c| c == '\r' || c == '\n'),
                    &mut cursor_total,
                );

                let mut chars = text.chars();
                let mut remaining = text.chars().count();

                while remaining > 0 {
                    let available = cursor_max - cursor;

                    if remaining <= available {
                        // It fits.
                        let text = chars.by_ref().take(remaining).collect::<String>();
                        cursor += remaining;

                        write!(handle, "{}", self.colorize.region(style, &text))?;
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
                        self.colorize.region(style, &text),
                        &panel_wrap,
                    )?;
                }
            }

            write!(handle, "\n")?;
        }

        Ok(())
    }
}
