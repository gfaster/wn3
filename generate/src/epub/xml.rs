use std::io::{self, prelude::*};

use crate::html_writer::EscapeAttr;

pub struct XmlSink<W> {
    w: W,
    queue: String,
}

static XML_HEADER: &str = r#"<?xml version="1.0"?>

"#;
static XHTML_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>

"#;

impl<W: Write> XmlSink<W> {
    pub fn new(mut w: W) -> io::Result<Self> {
        w.write_all(XML_HEADER.as_bytes())?;
        let ret = XmlSink {
            w,
            queue: String::new(),
        };
        Ok(ret)
    }

    pub fn new_xhtml(mut w: W) -> io::Result<Self> {
        w.write_all(XHTML_HEADER.as_bytes())?;
        let ret = XmlSink {
            w,
            queue: String::new(),
        };
        Ok(ret)
    }

    fn w(&mut self) -> io::Result<&mut W> {
        if !self.queue.is_empty() {
            self.w.write_all(self.queue.as_bytes())?;
            self.queue.clear();
        }
        Ok(&mut self.w)
    }

    pub fn mkel<'a>(
        &mut self,
        name: &'static str,
        attrs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> io::Result<Element<'_, W>> {
        self.write_el_start(name, attrs, false)?;
        Ok(Element { sink: self, name })
    }

    fn write_el_start<'a>(
        &mut self,
        name: &str,
        attrs: impl IntoIterator<Item = (&'a str, &'a str)>,
        self_closed: bool,
    ) -> io::Result<()> {
        let w = self.w()?;
        write!(w, "<{name}")?;
        for (attr, val) in attrs {
            let val = EscapeAttr(val);
            write!(w, r#" {attr}="{val}""#)?;
        }
        if self_closed {
            write!(w, " />")
        } else {
            write!(w, ">")
        }
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.w()?;
        Ok(())
    }
}

pub struct Element<'a, W> {
    sink: &'a mut XmlSink<W>,
    name: &'static str,
}

impl<'a, W: Write> Element<'a, W> {
    pub fn mkel<'b>(
        &'b mut self,
        name: &'static str,
        attrs: impl IntoIterator<Item = (&'b str, &'b str)>,
    ) -> io::Result<Element<'b, W>> {
        self.sink.write_el_start(name, attrs, false)?;
        Ok(Element {
            sink: self.sink,
            name,
        })
    }

    pub fn mkel_selfclosed<'b>(
        &mut self,
        name: &'static str,
        attrs: impl IntoIterator<Item = (&'b str, &'b str)>,
    ) -> io::Result<()> {
        self.sink.write_el_start(name, attrs, true)
    }

    pub fn write_lf(&mut self) -> io::Result<()> {
        writeln!(self.sink.w()?)
    }

    pub fn write_field(self, val: impl std::fmt::Display) -> io::Result<()> {
        write!(self.sink.w()?, "{val}")
    }
}

impl<W: Write> Write for Element<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sink.w()?.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sink.w()?.flush()
    }
}

impl<W> Drop for Element<'_, W> {
    fn drop(&mut self) {
        use std::fmt::Write;
        let Element { sink, name } = self;
        write!(sink.queue, "</{name}>").expect("writing into string is infalliable");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut out = Vec::new();
        let mut xml = XmlSink::new(&mut out).unwrap();
        let mut aaa = xml.mkel("aaa", [("attr", "val")]).unwrap();
        aaa.mkel("bbb", []).unwrap().write_field("ccc").unwrap();
        drop(aaa);
        xml.finish().unwrap();
        let expected = r#"<?xml version="1.0"?>

<aaa attr="val"><bbb>ccc</bbb></aaa>"#;
        assert_eq!(String::from_utf8_lossy(&out), expected);
    }
}
