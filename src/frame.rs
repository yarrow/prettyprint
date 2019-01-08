pub(crate) struct Frame {
    gutter: Option<&'static str>,
    term_width: usize,
    line_number_width: usize,
    separator_width: usize,
}

const LNUM_DIGITS: usize = 4;

impl Frame {
    pub(crate) fn new(term_width: usize, numbers: bool, grid: bool) -> Self {
        let (separator, separator_width) = if grid { (" │ ", 3) } else { (" ", 1) };
        let term_width_needed = LNUM_DIGITS + separator_width + 5;
        let (gutter, line_number_width, separator_width) = 
            if numbers && term_width >= term_width_needed {
                (Some(separator), LNUM_DIGITS, separator_width)
            } else {
                (None, 0, 0)
            };

        Frame {
            gutter,
            term_width,
            line_number_width,
            separator_width,
        }
    }

    pub(crate) fn horizontal_line(&self, grid_char: char) -> String {
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

    pub(crate) fn numbered_gutter(&mut self, line_number: usize) -> Option<String> {
        self.gutter.map(|separator| {
            let n = format!("{:4}", line_number);
            self.line_number_width = n.len();
            n + separator
        })
    }

    pub(crate) fn blank_gutter(&self) -> Option<String> {
        self.gutter
            .map(|separator| " ".repeat(self.line_number_width) + separator)
    }

    pub(crate) fn cursor_max(&self) -> usize {
        self.term_width - (self.line_number_width + self.separator_width)
    }
}

#[test]
fn gutter_prints_if_term_width_is_at_least_12() {
    let mut frame = Frame::new(12, true, true);
    assert!(frame.numbered_gutter(9999).is_some());

    let mut frame = Frame::new(11, true, true);
    assert!(frame.numbered_gutter(9999).is_none());
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
