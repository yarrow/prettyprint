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
    cached_wrap: DecorationText,
    cached_wrap_invalid_at: usize,
}

impl LineNumberDecoration {
    pub fn new() -> Self {
        LineNumberDecoration {
            cached_wrap_invalid_at: 10000,
            cached_wrap: DecorationText {
                text: " ".repeat(4),
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
                    text: " ".repeat(new_width),
                    width: new_width,
                };
            }

            self.cached_wrap.clone()
        } else {
            let plain: String = format!("{:4}", line_number);
            DecorationText {
                width: plain.len(),
                text: plain,
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
    pub fn new() -> Self {
        GridBorderDecoration {
            cached: DecorationText {
                text: "│".to_string(),
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
