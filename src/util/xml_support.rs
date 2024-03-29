//! These are helpers defined on top of `quick_xml` to make XML serialization
//! of `book` AST easier.
//!
//! The `xml_write!` macro is essentially a poor man's `Derive`.
//!
//! The code here was needed as no existing XML derive crate is complete enough to cover bard AST requirements.

use std::borrow::Cow;
use std::fmt::Display;
use std::fs::File;
use std::io;

use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Result as XmlResult;

pub type Writer<W = File> = quick_xml::Writer<W>;

type Map<K, V> = std::collections::BTreeMap<K, V>;

pub trait XmlWrite {
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write;
}

impl<'a, T> XmlWrite for &'a T
where
    T: XmlWrite + ?Sized,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        XmlWrite::write(*self, writer)
    }
}

impl<T> XmlWrite for Box<T>
where
    T: XmlWrite + ?Sized,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        T::write(self, writer)
    }
}

impl<'a, T> XmlWrite for Cow<'a, T>
where
    T: XmlWrite + Clone + ?Sized,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        match self {
            Cow::Borrowed(b) => b.write(writer),
            Cow::Owned(o) => o.write(writer),
        }
    }
}

macro_rules! impl_xmlwrite_text {
    ($ty:ty) => {
        impl XmlWrite for $ty {
            fn write<W>(&self, mut writer: &mut Writer<W>) -> XmlResult<()>
            where
                W: io::Write,
            {
                writer.write_text(self)
            }
        }
    };
    ($($ty:ty),+) => {
        $(impl_xmlwrite_text!($ty);)+
    };
}

impl_xmlwrite_text!(
    bool, char, u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, str, String
);

impl<I> XmlWrite for [I]
where
    I: XmlWrite,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        for item in self.iter() {
            XmlWrite::write(item, writer)?;
        }
        Ok(())
    }
}

impl<K, V> XmlWrite for Map<K, V>
where
    K: AsRef<str>,
    V: XmlWrite,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        for (k, v) in self.iter() {
            writer.tag(k.as_ref()).content()?.value(v)?.finish()?;
        }
        Ok(())
    }
}

impl XmlWrite for toml::Value {
    fn write<W>(&self, mut w: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        use toml::Value::*;

        match self {
            String(s) => w.write_text(s),
            Integer(i) => w.write_text(i),
            Float(f) => w.write_text(f),
            Boolean(b) => w.write_text(b),
            Datetime(dt) => w.write_text(dt),
            Array(ar) => {
                for item in ar.iter() {
                    w.tag("item").content()?.value(item)?.finish()?;
                }
                Ok(())
            }
            Table(t) => {
                for (k, v) in t.iter() {
                    w.tag(k.as_ref()).content()?.value(v)?.finish()?;
                }
                Ok(())
            }
        }
    }
}

pub struct Attr(String, String);

impl<N, V> From<(N, V)> for Attr
where
    N: ToString,
    V: ToString,
{
    fn from((name, value): (N, V)) -> Self {
        Self(name.to_string(), value.to_string())
    }
}

pub struct Field<T> {
    name: &'static str,
    value: T,
}

impl<T> Field<T> {
    pub fn new(name: &'static str, value: T) -> Self {
        Self { name, value }
    }

    pub fn unwrap(self) -> T {
        self.value
    }
}

impl<'a, T> Field<&'a Option<T>> {
    pub fn transpose(self) -> Option<Field<&'a T>> {
        let value = self.value.as_ref()?;
        Some(Field {
            name: self.name,
            value,
        })
    }
}

impl<T: ToString> From<Field<T>> for Attr {
    fn from(field: Field<T>) -> Self {
        Self(field.name.to_string(), field.value.to_string())
    }
}

impl<T, I> AsRef<[I]> for Field<T>
where
    T: AsRef<[I]>,
{
    fn as_ref(&self) -> &[I] {
        self.value.as_ref()
    }
}

impl<T> AsRef<str> for Field<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.value.as_ref()
    }
}

impl<T> XmlWrite for Field<T>
where
    T: XmlWrite,
{
    fn write<W>(&self, writer: &mut Writer<W>) -> XmlResult<()>
    where
        W: io::Write,
    {
        XmlWrite::write(&self.value, writer)
    }
}

pub struct TagBuilder<'w, W = File>
where
    W: io::Write,
{
    writer: &'w mut Writer<W>,
    name: String,
    attrs: Map<String, String>,
}

impl<'w, W> TagBuilder<'w, W>
where
    W: io::Write,
{
    pub fn attr(mut self, attr: impl Into<Attr>) -> Self {
        let Attr(name, value) = attr.into();
        self.attrs.insert(name, value);
        self
    }

    pub fn attr_opt(self, name: &str, attr: &Option<impl AsRef<str>>) -> Self {
        if let Some(attr) = attr {
            self.attr((name, attr.as_ref()))
        } else {
            self
        }
    }

    pub fn content(self) -> XmlResult<ContentBuilder<'w, W>> {
        let attrs = self
            .attrs
            .iter()
            .map(|(k, v)| Attribute::from((k.as_str(), v.as_str())));
        let elem = BytesStart::new(&self.name).with_attributes(attrs);
        self.writer.write_event(Event::Start(elem))?;

        Ok(ContentBuilder {
            writer: self.writer,
            parent_name: self.name,
        })
    }

    /// Creates an `<empty/>` tag.
    pub fn finish(self) -> XmlResult<()> {
        let attrs = self
            .attrs
            .iter()
            .map(|(k, v)| Attribute::from((k.as_str(), v.as_str())));
        let elem = BytesStart::new(&self.name).with_attributes(attrs);
        self.writer.write_event(Event::Empty(elem))
    }
}

pub struct ContentBuilder<'w, W = File>
where
    W: io::Write,
{
    writer: &'w mut Writer<W>,
    parent_name: String,
}

impl<'w, W> ContentBuilder<'w, W>
where
    W: io::Write,
{
    pub fn value(mut self, value: impl XmlWrite) -> XmlResult<Self> {
        self.writer.write_value(&value)?;
        Ok(self)
    }

    pub fn value_wrap(self, tag_name: &str, value: impl XmlWrite) -> XmlResult<Self> {
        self.writer
            .tag(tag_name)
            .content()?
            .value(value)?
            .finish()?;
        Ok(self)
    }

    pub fn field<T>(self, field: Field<T>) -> XmlResult<Self>
    where
        T: XmlWrite,
    {
        self.writer
            .tag(field.name)
            .content()?
            .value(&field.value)?
            .finish()?;
        Ok(self)
    }

    pub fn field_opt<T>(self, field: Field<&Option<T>>) -> XmlResult<Self>
    where
        T: XmlWrite,
    {
        if let Some(field) = field.transpose() {
            self.field(field)
        } else {
            Ok(self)
        }
    }

    pub fn many<I, T>(self, container: T) -> XmlResult<Self>
    where
        I: XmlWrite,
        T: AsRef<[I]>,
    {
        for item in container.as_ref().iter() {
            XmlWrite::write(item, self.writer)?;
        }

        Ok(self)
    }

    pub fn many_tags<I, T>(self, tag_name: &str, container: Field<T>) -> XmlResult<Self>
    where
        I: XmlWrite,
        T: AsRef<[I]>,
    {
        container
            .value
            .as_ref()
            .iter()
            .try_fold(self, |this, item| this.value_wrap(tag_name, item))
    }

    pub fn text(self, text: impl AsRef<str>) -> XmlResult<Self> {
        let text = BytesText::new(text.as_ref());
        self.writer.write_event(Event::Text(text))?;
        Ok(self)
    }

    pub fn comment(self, comment: impl AsRef<str>) -> XmlResult<Self> {
        let mut comment = comment.as_ref().replace("--", "- "); // extra space so that we don't get "--" out of "----"
        comment.insert(0, ' ');
        comment.push(' ');
        let comment = BytesText::from_escaped(comment);
        self.writer.write_event(Event::Comment(comment))?;
        Ok(self)
    }

    pub fn finish(self) -> XmlResult<()> {
        let elem = BytesEnd::new(&self.parent_name);
        self.writer.write_event(Event::End(elem))?;

        Ok(())
    }
}

pub trait WriterExt<'w, W = File>
where
    W: io::Write,
{
    fn tag(self, name: &str) -> TagBuilder<'w, W>;
    fn write_value(&mut self, value: &impl XmlWrite) -> XmlResult<()>;
    fn write_text(&mut self, text: &(impl Display + ?Sized)) -> XmlResult<()>;
}

impl<'w, W> WriterExt<'w, W> for &'w mut Writer<W>
where
    W: io::Write,
{
    fn tag(self, name: &str) -> TagBuilder<'w, W> {
        TagBuilder {
            writer: self,
            name: name.to_string(),
            attrs: Map::new(),
        }
    }

    fn write_value(&mut self, value: &impl XmlWrite) -> XmlResult<()> {
        XmlWrite::write(value, self)
    }

    fn write_text(&mut self, text: &(impl Display + ?Sized)) -> XmlResult<()> {
        let text = format!("{}", text);
        self.write_event(Event::Text(BytesText::new(&text)))
    }
}

#[macro_export]
macro_rules! xml_write {
    (struct $ty:ident $(<$life:lifetime>)? { $($field:ident ,)+ } -> |$writer:ident| $block:block) => {
        impl $(<$life>)? XmlWrite for $ty $(<$life>)? {
            fn write<W>(&self, $writer: &mut Writer<W>) -> quick_xml::Result<()>
            where
                W: ::std::io::Write
            {
                let $ty { $($field,)+ } = self;
                $( let $field = Field::new(stringify!($field), $field); )+
                $block.finish()
            }
        }
    };

    (enum $ty:ident |$writer:ident| { $($var:pat => $block:block ,)+ } ) => {
        impl XmlWrite for $ty {
            fn write<W>(&self, mut $writer: &mut Writer<W>) -> quick_xml::Result<()>
            where
                W: ::std::io::Write
            {
                use $ty::*;
                match self {
                    $($var => { $block })+
                }

                Ok(())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_comment() {
        let buffer = vec![];
        let mut writer = Writer::new(buffer);

        writer
            .tag("test")
            .content()
            .unwrap()
            .comment("double dashes are illegal -- ---- -------- xml is pretty weird")
            .unwrap()
            .finish()
            .unwrap();

        let buffer = writer.into_inner();
        let xml = String::from_utf8(buffer).unwrap();
        let xml = xml.replace("!--", "").replace("-->", "");
        assert!(xml.contains("double dashes are illegal"));
        assert!(!xml.contains("--"));
    }
}
