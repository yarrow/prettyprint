/// This module exists to ensure that refactoring the decoration module doesn't
/// break it.  Decorations are the items that `PrettyPrinter` adds to the output:
/// line numbers, the filename or other header, and the grid lines that separate
/// those things from the colored source code.
extern crate serde;
extern crate serde_json;

use assets::HighlightingAssets;
use content_inspector::ContentType;
use errors::*;
use inputfile::{InputFile, InputFileReader};
use printer::{ColorProtocol, InteractivePrinter, Printer};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
};
use style::{OutputComponent, OutputComponents, OutputWrap};
use syntect::highlighting;

#[test]
fn test_line_wrap() {
    const LINE: &str = r#"
const LINE: &str = "abc defghijklmno pqrs tuv wxyz";
"#;
    const EXPECTED: &str = r#"   1 │ 
   2 │ const LINE: &
     │ str = "abc de
     │ fghijklmno pq
     │ rs tuv wxyz";
─────┴──────────────
"#;
    let assets = HighlightingAssets::new();
    let settings = PrintSettings {
        content_type: ContentType::UTF_8,
        grid: true,
        header: false,
        line_numbers: true,
        colored_output: false,
        true_color: false,
        term_width: 20,
        tab_width: 4,
        show_nonprintable: false,
        output_wrap: OutputWrap::Character,
        use_italic_text: false,
        header_overwrite: false,
        gutter_color: None,
    };
    let result = output_for(&assets, &settings, LINE);
    println!("{}", result);
    assert_eq!(result, EXPECTED);
}

fn output_for(assets: &HighlightingAssets, settings: &PrintSettings, input: &str) -> String {
    let input = InputFile::String(input.to_string());
    let mut reader = input.get_reader().unwrap();

    let theme = theme_with_gutter(&assets, settings.gutter_color);
    let mut printer = a_printer(&assets, &theme, &settings);

    let output = pretty_printed(&mut reader, &mut printer, settings.header_overwrite).unwrap();
    String::from_utf8(output).unwrap()
}

/// First we test that a few representative settings produce the same result as
/// the original code
#[test]
fn ansi_samples_are_same_as_original() {
    must_be_equal_to_stored_results(sample_test_cases(), "fixtures/sample-ansi-results.json");
}

// Use `equiv` when making a change that causes new values to disagree with
// the saved expected values, but they are equivalent under some transformation.
//
// Mostly it will just be simple equality:
fn equiv(a: &String, b: &String) -> bool {
    *a == *b
}

// Here's an example that I used when making a change that affected where ANSI codes were placed in
// a sequence of blanks:
/*
fn equiv(a: &String, b: &String) -> bool {
    a.len() == b.len() && munge(a) == munge(b)
}
fn munge(s: &String) -> String {
    let mut s = s.clone();
    s.retain(|c| c != ' ');
    s
}
*/
type TestResult = HashMap<String, String>;
fn must_be_equal_to_stored_results(actual: TestResult, expected: &str) {
    // let new_json = serde_json::to_string_pretty(&actual).unwrap();
    // fs::write("results.json", new_json).unwrap();
    let json = String::from_utf8(fs::read(expected).unwrap()).unwrap();
    let expected: TestResult = serde_json::from_str(&json).unwrap();
    assert_eq!(actual.len(), expected.len());
    for key in expected.keys() {
        let a = &actual[key];
        let e = &expected[key];
        if !equiv(a, e) {
            println!(
                "Actual != expected for {}\nActual:\n{}\nExpected:\n{}\n",
                key, a, e
            );
            assert_eq!(a, e, "\nsettings are: {}\n", key);
        }
    }
}

/// The sample test cases take reasonable default settings and change each
/// individual setting one by one.
fn sample_test_cases() -> TestResult {
    let assets = HighlightingAssets::new();
    let mut results = TestResult::new();
    let default = PrintSettings {
        content_type: ContentType::UTF_8,
        grid: true,
        header: true,
        line_numbers: true,
        colored_output: true,
        true_color: true,
        term_width: 100,
        tab_width: 4,
        show_nonprintable: false,
        output_wrap: OutputWrap::None,
        use_italic_text: false,
        header_overwrite: false,
        gutter_color: None,
    };
    let wrapped = PrintSettings {
        output_wrap: OutputWrap::Character,
        ..default
    };

    #[rustfmt::skip]
    let tests = [
        PrintSettings { ..default },
        PrintSettings { content_type: ContentType::BINARY, ..default },
        PrintSettings { grid: false, ..default },
        PrintSettings { header: false, ..default },
        PrintSettings { line_numbers: false, ..default },
        PrintSettings { colored_output: false, true_color: false, ..default },
        PrintSettings { true_color: false, ..default },
        PrintSettings { term_width: 10, ..default },
        PrintSettings { tab_width: 0, ..default },
        PrintSettings { tab_width: 8, ..default },
        PrintSettings { show_nonprintable: true, ..default },
        PrintSettings { use_italic_text: true, ..default },
        PrintSettings { header_overwrite: true, ..default },
        PrintSettings { true_color: true, gutter_color: MAGENTA, ..default },
        PrintSettings { true_color: false, gutter_color: MAGENTA, ..default },
        PrintSettings { ..wrapped },
        PrintSettings { term_width: 10, ..wrapped },
        PrintSettings { term_width: 10, grid: false, ..wrapped },
    ];

    for settings in tests.iter() {
        let (key, result) = test_with(&assets, settings);
        results.insert(key, result);
    }
    results
}

// This magenta is not generated by any of the 256 ANSI color codes.
const MAGENTA: Option<highlighting::Color> = Some(highlighting::Color {
    r: 255,
    g: 0,
    b: 235,
    a: 0,
});

const FIB: &str = "
pub fn fib(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => fib(n - 1) + fib(n - 2),
    }
}
";

fn pretty_fib(assets: &HighlightingAssets, settings: &PrintSettings) -> String {
    output_for(&assets, &settings, FIB)
}

fn test_with(assets: &HighlightingAssets, settings: &PrintSettings) -> (String, String) {
    let result = pretty_fib(&assets, &settings);
    let key: String = format!("{}", settings);
    //                                                println!("{}:\n{}", key, result);
    (key, result)
}

fn pretty_printed<'a, P: Printer>(
    reader: &mut InputFileReader,
    printer: &mut P,
    header_overwrite: bool,
) -> Result<Vec<u8>> {
    let mut output: Vec<u8> = b"".to_vec();

    // Fragile! Only works because print_header only looks at the file name in
    // InputFile::Ordinary(filename)
    const FERRIS: &str = "Ferris was here";
    let (fname, title) = if header_overwrite {
        ("", Some(FERRIS.to_string()))
    } else {
        ("Fer.rs", None)
    };
    printer.print_header(&mut output, &InputFile::Ordinary(fname.to_string()), title)?;

    let mut line_buffer = Vec::new();
    let mut line_number: usize = 1;
    while reader.read_line(&mut line_buffer)? {
        printer.print_line(false, &mut output, line_number, &line_buffer)?;
        line_number += 1;
        line_buffer.clear();
    }

    printer.print_footer(&mut output)?;

    Ok(output)
}

fn theme_with_gutter(
    assets: &HighlightingAssets,
    color: Option<highlighting::Color>,
) -> highlighting::Theme {
    const TEST_THEME: &str = "Monokai Extended"; // prettyprint's default at the time of writing
    let mut theme = assets.get_theme(TEST_THEME).clone();
    theme.settings.gutter_foreground = color;
    theme
}

fn a_printer<'a>(
    assets: &'a HighlightingAssets,
    theme: &'a highlighting::Theme,
    s: &PrintSettings,
) -> InteractivePrinter<'a> {
    let syntax_set = &assets.syntax_set;
    let syntax = syntax_set.find_syntax_by_token("rust").unwrap();

    let colorize_to = if !s.colored_output {
        ColorProtocol::Plain
    } else {
        ColorProtocol::Terminal {
            true_color: s.true_color,
            use_italic_text: s.use_italic_text,
        }
    };

    InteractivePrinter::new2(
        &theme,
        syntax,
        syntax_set,
        s.content_type,
        get_output_components(s.grid, s.header, s.line_numbers),
        colorize_to,
        s.gutter_color,
        s.term_width,
        s.tab_width,
        s.show_nonprintable,
        s.output_wrap,
    )
}

fn get_output_components(grid: bool, header: bool, line_numbers: bool) -> OutputComponents {
    let mut components = HashSet::new();
    if grid {
        components.insert(OutputComponent::Grid);
    }
    if header {
        components.insert(OutputComponent::Header);
    }
    if line_numbers {
        components.insert(OutputComponent::Numbers);
    }
    OutputComponents(components)
}

struct PrintSettings {
    content_type: ContentType,
    grid: bool,
    header: bool,
    line_numbers: bool,
    colored_output: bool,
    true_color: bool,
    term_width: usize,
    tab_width: usize,
    show_nonprintable: bool,
    output_wrap: OutputWrap,
    use_italic_text: bool,
    header_overwrite: bool,
    gutter_color: Option<highlighting::Color>,
}

/// We implement Display so we can have human-readable keys in TestResult
impl fmt::Display for PrintSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut c = String::new();
        if !self.content_type.is_text() {
            c += "binary,"
        }
        if self.grid {
            c += "grid,"
        }
        if self.header {
            c += "header,"
        }
        if self.line_numbers {
            c += "line_numbers,"
        }
        if self.show_nonprintable {
            c += "show_nonprintable,"
        }
        if self.output_wrap != OutputWrap::None {
            c += "output_wrap,"
        }
        if self.use_italic_text {
            c += "use_italic_text,"
        }
        if self.header_overwrite {
            c += "header_overwrite,"
        }
        if self.gutter_color.is_some() {
            c += "garish,";
        }
        c += if !self.colored_output {
            "color=none,"
        } else if self.true_color {
            "color=true,"
        } else {
            "color=limited,"
        };
        write!(
            f,
            "{}term_width={},tab_width={}",
            c, self.term_width, self.tab_width
        )
    }
}

#[test]
#[ignore] // Too expensive for routine testing -- almost 14,000 combinations
fn long_ansi_is_same_as_orginal() {
    must_be_equal_to_stored_results(all_test_cases(), "fixtures/ansi-results.json");
}

fn all_test_cases() -> TestResult {
    const BOOLEANS: &[bool; 2] = &[false, true];
    let assets = HighlightingAssets::new();
    let mut results = TestResult::new();
    #[rustfmt::skip]
    for (colored_output, true_color) in &[(false, false), (true, false), (true, true)] {
      for content_type in &[ContentType::UTF_8, ContentType::BINARY] {
        for output_wrap in &[OutputWrap::None, OutputWrap::Character] {
          for gutter_color in &[None, MAGENTA] {
            for grid in BOOLEANS {
              for header in BOOLEANS {
                for line_numbers in BOOLEANS {
                  for show_nonprintable in BOOLEANS {
                    for use_italic_text in BOOLEANS {
                      for term_width in &[4, 10, 100] {
                        for tab_width in &[0, 4, 8] {
                          for header_overwrite in BOOLEANS {
                            let settings = PrintSettings {
                                content_type: *content_type,
                                grid: *grid,
                                header: *header,
                                line_numbers: *line_numbers,
                                colored_output: *colored_output,
                                true_color: *true_color,
                                term_width: *term_width,
                                tab_width: *tab_width,
                                show_nonprintable: *show_nonprintable,
                                output_wrap: *output_wrap,
                                use_italic_text: *use_italic_text,
                                header_overwrite: *header_overwrite,
                                gutter_color: *gutter_color,
                            };
                            let (key, result) = test_with(&assets, &settings);
                            results.insert(key, result);
                            //println!("{}:\n{}", key, output);
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    };
    results
}
