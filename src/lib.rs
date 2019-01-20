#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
#![deny(unused_must_use)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::stutter)]
#![allow(clippy::similar_names)]
#![allow(clippy::let_and_return)]
#![allow(clippy::pub_enum_variant_names)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::use_self)]
#![allow(clippy::single_match_else)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::result_map_unwrap_or_else)]
#![allow(clippy::non_ascii_literal)]

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::extra_unused_lifetimes)]
#![allow(clippy::or_fun_call)]

//#![warn(missing_docs)]

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use]
extern crate derive_builder;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate lazy_static;

extern crate ansi_term;
extern crate atty;
extern crate console;
extern crate content_inspector;
extern crate directories;
extern crate encoding;
extern crate shell_words;
extern crate syntect;

mod assets;
mod builder;
mod colorize;
mod dirs;
mod frame;
mod inputfile;
mod line_range;
mod output;
mod preprocessor;
mod printer;
mod style;
mod syntax_mapping;

#[cfg(test)]
mod test_ansi_code_preservation;

pub use builder::{PagingMode, PrettyPrint, PrettyPrinter};
// pub use style::OutputComponent;

mod errors {
    error_chain! {
        foreign_links {
            Clap(::clap::Error);
            Io(::std::io::Error);
            SyntectError(::syntect::LoadingError);
            ParseIntError(::std::num::ParseIntError);
        }
    }
}

pub use errors::Error as PrettyPrintError;

#[cfg(test)]
mod tests {
    use super::*;

    /// Pretty prints its own code
    #[test]
    fn it_works() {
        // PagingMode::Never because otherwise `cargo watch -x test` hangs.
        let printer = PrettyPrinter::default().paging_mode(PagingMode::Never).build().unwrap();
        printer.file("fixtures/fib.rs").unwrap();
    }

    /// Pretty prints its own code with some more formatting shenanigans
    #[test]
    fn it_works_with_output_opts() {
        let printer = PrettyPrinter::default()
            .line_numbers(true)
            .header(true)
            .grid(true)
            .paging_mode(PagingMode::Never)
            .language("ruby")
            .build()
            .unwrap();

        let example = r#"
        def fib(n)        
            return 1 if n <= 1
            fib(n-1) + fib(n-2)
        end
        "#;
        printer.string_with_header(example, "example.rb").unwrap();
    }

    /// Show available syntax highlighting themes
    #[test]
    fn show_themes() {
        let printer = PrettyPrinter::default().build().unwrap();
        assert!(printer.get_themes().len() > 0);
        println!("{:?}", printer.get_themes().keys());
    }
}
