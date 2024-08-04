use std::{
    fmt::{self, Display},
    iter::{self, FusedIterator},
};

pub struct NopDisplay;
impl Display for NopDisplay {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct Join<I, Sep> {
    sep: Sep,
    items: I,
}

impl<D, Sep, I> Display for Join<I, Sep>
where
    I: IntoIterator<Item = D> + Clone + Copy,
    D: Display,
    Sep: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Join { items, ref sep } = *self;
        let mut it = items.into_iter();
        let Some(first) = it.next() else {
            return Ok(());
        };
        first.fmt(f)?;
        for item in it {
            write!(f, "{sep}")?;
            item.fmt(f)?;
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub trait DispJoin<'a>
where
    Self: 'a,
    &'a Self: IntoIterator,
    <&'a Self as IntoIterator>::Item: Display,
{
    fn disp_join<Sep: Display>(&'a self, sep: Sep) -> Join<&'a Self, Sep>;
}

impl<'a, D: Display + 'a> DispJoin<'a> for [D] {
    fn disp_join<Sep: Display>(&'a self, sep: Sep) -> Join<&'a Self, Sep> {
        Join { sep, items: self }
    }
}

pub struct MapJoin<I, Sep, F> {
    sep: Sep,
    items: I,
    func: F,
}

impl<D, Sep, I, F> Display for MapJoin<I, Sep, F>
where
    I: IntoIterator + Clone + Copy,
    F: Fn(I::Item) -> D,
    D: Display,
    Sep: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let MapJoin {
            items,
            ref sep,
            ref func,
        } = *self;
        let mut it = items.into_iter();
        let Some(first) = it.next() else {
            return Ok(());
        };
        func(first).fmt(f)?;
        for item in it {
            write!(f, "{sep}")?;
            func(item).fmt(f)?;
        }
        Ok(())
    }
}

pub trait MapDispJoin<'a>
where
    Self: 'a,
    &'a Self: IntoIterator,
{
    fn map_disp_join<Sep, F, D>(&'a self, sep: Sep, f: F) -> MapJoin<&'a Self, Sep, F>
    where
        Sep: Display,
        D: Display,
        F: Fn(<&'a Self as IntoIterator>::Item) -> D;
}

impl<'a, T> MapDispJoin<'a> for T
where
    T: 'a,
    &'a T: IntoIterator,
{
    fn map_disp_join<Sep, F, D>(&'a self, sep: Sep, f: F) -> MapJoin<&T, Sep, F>
    where
        Sep: Display,
        D: Display,
        F: Fn(<&'a Self as IntoIterator>::Item) -> D,
    {
        MapJoin {
            sep,
            items: self,
            func: f,
        }
    }
}

// impl<I> DispJoin for I
//     where
//     I: IntoIterator + Clone + Copy,
//     <I as IntoIterator>::Item: Display
// {
//     fn disp_join(self, sep: &'static str) -> Join<Self> {
//         Join {
//             items: self,
//             sep,
//         }
//     }
// }

pub trait StrArr<'a>: Clone + Copy {
    type StrIt: Iterator<Item = &'a str> + DoubleEndedIterator + ExactSizeIterator + FusedIterator;

    fn str_arr(self) -> Self::StrIt;
}

impl<'a> StrArr<'a> for &'a [&'a str] {
    type StrIt = iter::Copied<std::slice::Iter<'a, &'a str>>;

    fn str_arr(self) -> Self::StrIt {
        self.iter().copied()
    }
}

impl<'a> StrArr<'a> for &'a str {
    type StrIt = iter::Once<&'a str>;

    fn str_arr(self) -> Self::StrIt {
        iter::once(self)
    }
}

#[derive(Clone, Copy)]
pub struct TagSurround<D, A> {
    tags: A,
    content: D,
}

impl<'a, D, A: StrArr<'a>> TagSurround<D, A> {
    pub fn new(tags: A, content: D) -> Self {
        Self { tags, content }
    }
}

impl<'a, D: Display, A: StrArr<'a>> Display for TagSurround<D, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tag in self.tags.str_arr() {
            debug_assert!(!tag.is_empty(), "cannot have empty tags");
            write!(f, "<{tag}>")?;
        }
        write!(f, "{}", self.content)?;
        for tag in self.tags.str_arr().rev() {
            write!(f, "</{tag}>")?;
        }
        Ok(())
    }
}

pub struct Surround<'a, D> {
    open: &'a str,
    close: &'a str,
    content: D,
}

impl<'a, D: Display> Display for Surround<'a, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Surround {
            open,
            close,
            ref content,
        } = *self;
        write!(f, "{open}{content}{close}")
    }
}

#[allow(dead_code)]
pub trait SurroundExt: Display + Sized {
    fn surround<'a>(self, open: &'a str, close: &'a str) -> Surround<'a, Self>;
    fn surround_tag<'a, A: StrArr<'a>>(self, tags: A) -> TagSurround<Self, A>;
}

impl<T: Display + Sized> SurroundExt for T {
    fn surround<'a>(self, open: &'a str, close: &'a str) -> Surround<'a, Self> {
        Surround {
            open,
            close,
            content: self,
        }
    }

    fn surround_tag<'a, A: StrArr<'a>>(self, tags: A) -> TagSurround<Self, A> {
        TagSurround {
            tags,
            content: self,
        }
    }
}

#[derive(Clone, Copy)]
pub struct EscapeBody<'a>(pub &'a str);

impl Display for EscapeBody<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stole from https://doc.rust-lang.org/beta/nightly-rustc/src/rustdoc/html/escape.rs.html
        let EscapeBody(s) = *self;
        let raw = s;
        let mut last = 0;
        for (i, ch) in s.char_indices() {
            let s = match ch {
                '>' => "&gt;",
                '<' => "&lt;",
                '&' => "&amp;",
                // '\u{00a0}' => "&nbsp;",
                _ => continue,
            };
            f.write_str(&raw[last..i])?;
            f.write_str(s)?;
            last = i + 1;
        }

        if last < s.len() {
            f.write_str(&raw[last..])?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct EscapeAttr<'a>(pub &'a str);

impl Display for EscapeAttr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stole from https://doc.rust-lang.org/beta/nightly-rustc/src/rustdoc/html/escape.rs.html
        let EscapeAttr(s) = *self;
        let raw = s;
        let mut last = 0;
        for (i, ch) in s.char_indices() {
            let s = match ch {
                '>' => "&gt;",
                '<' => "&lt;",
                '&' => "&amp;",
                '\"' => "\\\"",
                _ => continue,
            };
            f.write_str(&raw[last..i])?;
            f.write_str(s)?;
            last = i + 1;
        }

        if last < s.len() {
            f.write_str(&raw[last..])?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct EscapeMd<'a>(pub &'a str);

impl Display for EscapeMd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stole from https://doc.rust-lang.org/beta/nightly-rustc/src/rustdoc/html/escape.rs.html
        let EscapeMd(s) = *self;
        let raw = s;
        let mut last = 0;
        for (i, ch) in s.char_indices() {
            let s = match ch {
                '`' => "\\`",
                '\\' => "\\\\",
                '*' => "\\*",
                '[' => "\\[",
                ']' => "\\]",
                _ => continue,
            };
            f.write_str(&raw[last..i])?;
            f.write_str(s)?;
            last = i + 1;
        }

        if last < s.len() {
            f.write_str(&raw[last..])?;
        }
        Ok(())
    }
}

// macro_rules! const_sep {
//     ($s:expr) => {
//         {
//             struct D;
//             impl std::fmt::Display for D {
//                 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//                     f.write_str($s)
//                 }
//             }
//             D
//         }
//     };
// }
// pub(crate) use const_sep;
