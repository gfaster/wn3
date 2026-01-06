#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpanStyle {
    pub(super) bold: bool,
    // TODO: distinguish between italics and emphasis (I learned after I originally wrote this that
    // they are in fact different)
    pub(super) italic: bool,
    pub(super) footnote: bool,
    pub(super) ruby: bool,
    pub(super) ruby_rt: bool,
    pub(super) ruby_rp: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpanStyleEl {
    Bold,
    Italic,
    Ruby,
    RubyRt,
    RubyRp,
    Footnote,
}

impl SpanStyleEl {
    pub(super) fn open(self) -> &'static str {
        match self {
            SpanStyleEl::Bold => "<b>",
            SpanStyleEl::Italic => "<i>",
            SpanStyleEl::Footnote => r#"<aside role="doc-footnote">"#,
            SpanStyleEl::Ruby => "<ruby>",
            SpanStyleEl::RubyRt => "<rt>",
            SpanStyleEl::RubyRp => "<rp>",
        }
    }

    pub(super) fn close(self) -> &'static str {
        match self {
            SpanStyleEl::Bold => "</b>",
            SpanStyleEl::Italic => "</i>",
            SpanStyleEl::Footnote => "</aside>",
            SpanStyleEl::Ruby => "</ruby>",
            SpanStyleEl::RubyRt => "</rt>",
            SpanStyleEl::RubyRp => "</rp>",
        }
    }
}

impl From<SpanStyleEl> for SpanStyle {
    fn from(value: SpanStyleEl) -> Self {
        match value {
            SpanStyleEl::Bold => SpanStyle {
                bold: true,
                ..SpanStyle::none()
            },
            SpanStyleEl::Italic => SpanStyle {
                italic: true,
                ..SpanStyle::none()
            },
            SpanStyleEl::Footnote => SpanStyle {
                footnote: true,
                ..SpanStyle::none()
            },
            SpanStyleEl::Ruby => SpanStyle {
                ruby: true,
                ..SpanStyle::none()
            },
            SpanStyleEl::RubyRt => SpanStyle {
                ruby_rt: true,
                ..SpanStyle::none()
            },
            SpanStyleEl::RubyRp => SpanStyle {
                ruby_rp: true,
                ..SpanStyle::none()
            },
        }
    }
}

impl SpanStyle {
    pub const fn none() -> Self {
        SpanStyle {
            bold: false,
            italic: false,
            footnote: false,
            ruby: false,
            ruby_rt: false,
            ruby_rp: false,
        }
    }

    pub const fn is_none(self) -> bool {
        matches!(
            self,
            SpanStyle {
                bold: false,
                italic: false,
                footnote: false,
                ruby: false,
                ruby_rt: false,
                ruby_rp: false,
            }
        )
    }

    pub fn el_iter(self) -> impl DoubleEndedIterator<Item = SpanStyleEl> {
        [
            self.bold.then_some(SpanStyleEl::Bold),
            self.italic.then_some(SpanStyleEl::Italic),
            self.footnote.then_some(SpanStyleEl::Footnote),
            self.ruby.then_some(SpanStyleEl::Ruby),
            self.ruby_rt.then_some(SpanStyleEl::RubyRt),
            self.ruby_rp.then_some(SpanStyleEl::RubyRp),
        ]
        .into_iter()
        .flatten()
    }

    pub const fn bold() -> Self {
        SpanStyle {
            bold: true,
            ..Self::none()
        }
    }

    pub const fn italic() -> Self {
        SpanStyle {
            italic: true,
            ..Self::none()
        }
    }

    pub const fn ruby() -> Self {
        SpanStyle {
            ruby: true,
            ..Self::none()
        }
    }

    pub const fn ruby_rt() -> Self {
        SpanStyle {
            ruby_rt: true,
            ..Self::none()
        }
    }

    pub const fn ruby_rp() -> Self {
        SpanStyle {
            ruby_rp: true,
            ..Self::none()
        }
    }

    pub const fn bold_italic() -> Self {
        SpanStyle {
            bold: true,
            italic: true,
            ..Self::none()
        }
    }

    /// styles needed to be enabled to get to `to`
    pub(super) const fn additional_needed(self, to: Self) -> Self {
        macro_rules! fields {
            ($($field:ident)*) => {
                Self {
                    $($field: !self.$field & to.$field,)*
                }
            };
        }
        fields!(bold italic footnote ruby ruby_rt ruby_rp)
    }

    /// styles needed to be disabled to get to `to`
    pub(super) const fn removals_needed(self, to: Self) -> Self {
        macro_rules! fields {
            ($($field:ident)*) => {
                Self {
                    $($field: self.$field & !to.$field,)*
                }
            };
        }
        fields!(bold italic footnote ruby ruby_rt ruby_rp)
    }
}

impl FromIterator<SpanStyleEl> for SpanStyle {
    fn from_iter<T: IntoIterator<Item = SpanStyleEl>>(iter: T) -> Self {
        iter.into_iter().fold(SpanStyle::none(), |acc, x| acc + x)
    }
}

impl std::ops::Add for SpanStyle {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        macro_rules! fields {
            ($($field:ident)*) => {
                Self {
                    $($field: self.$field | rhs.$field,)*
                }
            };
        }
        fields!(bold italic footnote ruby ruby_rt ruby_rp)
    }
}

impl std::ops::AddAssign for SpanStyle {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

impl std::ops::Add<SpanStyleEl> for SpanStyle {
    type Output = Self;

    fn add(self, rhs: SpanStyleEl) -> Self::Output {
        self + Self::from(rhs)
    }
}

impl std::ops::AddAssign<SpanStyleEl> for SpanStyle {
    fn add_assign(&mut self, rhs: SpanStyleEl) {
        *self = *self + rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParagraphStyle {
    pub mode: ParagraphMode,
    pub align: Align,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphMode {
    #[default]
    Normal,
    BlockQuote,
}
