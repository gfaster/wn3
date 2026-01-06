use std::fmt::{self, Display};

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
    fn map_disp_join<Sep, F, D>(&self, sep: Sep, f: F) -> MapJoin<&T, Sep, F>
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

pub struct TupleJoin<Tuple>(Tuple);

impl<T: Display, U: Display> Display for TupleJoin<(T, U)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (t, u) = &self.0;
        write!(f, "{t}{u}")
    }
}

impl<T: Display, U: Display> Display for TupleJoin<&(T, U)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (t, u) = &self.0;
        write!(f, "{t}{u}")
    }
}

impl<T: Display, U: Display, V: Display> Display for TupleJoin<(T, U, V)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (t, u, v) = &self.0;
        write!(f, "{t}{u}{v}")
    }
}

impl<T: Display, U: Display, V: Display> Display for TupleJoin<&(T, U, V)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (t, u, v) = &self.0;
        write!(f, "{t}{u}{v}")
    }
}

#[allow(dead_code)]
pub trait TupleDispJoin: Sized {
    fn tuple_display_join(self) -> TupleJoin<Self>;
}

impl<T: Display, U: Display> TupleDispJoin for (T, U) {
    fn tuple_display_join(self) -> TupleJoin<Self> {
        TupleJoin(self)
    }
}
impl<T: Display, U: Display, V: Display> TupleDispJoin for (T, U, V) {
    fn tuple_display_join(self) -> TupleJoin<Self> {
        TupleJoin(self)
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

#[derive(Clone, Copy)]
pub struct TagSurround<D, A> {
    tags: A,
    content: D,
}

impl<D: Display, T: Tag> Display for TagSurround<D, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.tags.open(),
            self.content,
            self.tags.close()
        )
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

// type Surround<'a, D> = SurroundDisp<&'a str, &'a str, D>;

pub struct SurroundDisp<Open, Close, Content> {
    open: Open,
    close: Close,
    content: Content,
}

impl<Open: Display, Close: Display, Content: Display> Display
    for SurroundDisp<Open, Close, Content>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let SurroundDisp {
            open,
            close,
            content,
        } = self;
        write!(f, "{open}{content}{close}")
    }
}

#[allow(dead_code)]
pub trait SurroundExt: Display + Sized {
    fn surround<'a>(self, open: &'a str, close: &'a str) -> Surround<'a, Self>;
    fn surround_disp<O: Display, C: Display>(self, open: O, close: C) -> SurroundDisp<O, C, Self>;
    fn surround_tag<'a, T: Tag>(self, tags: T) -> TagSurround<Self, T>;
}

impl<D: Display + Sized> SurroundExt for D {
    fn surround<'a>(self, open: &'a str, close: &'a str) -> Surround<'a, Self> {
        Surround {
            open,
            close,
            content: self,
        }
    }

    fn surround_tag<'a, T: Tag>(self, tags: T) -> TagSurround<Self, T> {
        TagSurround {
            tags,
            content: self,
        }
    }

    fn surround_disp<O: Display, C: Display>(self, open: O, close: C) -> SurroundDisp<O, C, Self> {
        SurroundDisp {
            open,
            close,
            content: self,
        }
    }
}

struct WrapClose<'a>(&'a str);
impl Display for WrapClose<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "</{}>", self.0)
    }
}

pub trait Tag {
    fn open(&self) -> impl Display + '_;
    fn close(&self) -> impl Display + '_;
}

impl Tag for &str {
    fn open(&self) -> impl Display + '_ {
        struct D<'a>(&'a str);
        impl Display for D<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "<{}>", self.0)
            }
        }
        D(self)
    }

    fn close(&self) -> impl Display + '_ {
        WrapClose(&self)
    }
}

/// warning: does not escape!
///
/// I'm not at all convinced this is not slower
pub struct Tag2<A1, A2> {
    name: &'static str,
    k1: &'static str,
    a1: A1,
    k2: &'static str,
    a2: A2,
}

impl<A1: Display, A2: Display> Tag for Tag2<A1, A2> {
    fn open(&self) -> impl Display + '_ {
        struct D<'a, A1, A2>(&'a Tag2<A1, A2>);
        impl<A1: Display, A2: Display> Display for D<'_, A1, A2> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let Tag2 {
                    name,
                    k1,
                    a1,
                    k2,
                    a2,
                } = self.0;
                write!(f, r#"<{name} {k1}="{a1}" {k2}="{a2}">"#)
            }
        }
        D(self)
    }

    fn close(&self) -> impl Display + '_ {
        WrapClose(self.name)
    }
}

impl<A1: Display, A2: Display> Tag2<A1, A2> {
    #[expect(dead_code)]
    pub const fn new(
        name: &'static str,
        k1: &'static str,
        a1: A1,
        k2: &'static str,
        a2: A2,
    ) -> Self {
        Self {
            name,
            k1,
            a1,
            k2,
            a2,
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
