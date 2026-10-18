//! Hugging Face overrides Jinja's HTML-safe tojson with Python json.dumps.
//! Preserve order, Unicode, spacing and number representation explicitly.
use crate::render::LimitedBuffer;
use minijinja::{
    Error, ErrorKind, Value,
    value::{Kwargs, ValueKind},
};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::ser::Formatter;
use std::io::{self, Write};

struct Ordered<'a> {
    value: &'a Value,
    sort: bool,
    depth: usize,
}
impl Serialize for Ordered<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        if self.depth > 64 {
            return Err(S::Error::custom("JSON depth"));
        }
        match self.value.kind() {
            ValueKind::Map => {
                let mut keys = self
                    .value
                    .try_iter()
                    .map_err(S::Error::custom)?
                    .map(|key| {
                        key.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| S::Error::custom("string JSON keys required"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if self.sort {
                    keys.sort();
                }
                let mut map = serializer.serialize_map(Some(keys.len()))?;
                for key in keys {
                    let value = self
                        .value
                        .get_item(&Value::from(key.as_str()))
                        .map_err(S::Error::custom)?;
                    map.serialize_entry(
                        &key,
                        &Ordered {
                            value: &value,
                            sort: self.sort,
                            depth: self.depth + 1,
                        },
                    )?;
                }
                map.end()
            }
            ValueKind::Seq => {
                let values = self.value.try_iter().map_err(S::Error::custom)?;
                let mut seq = serializer.serialize_seq(None)?;
                for value in values {
                    seq.serialize_element(&Ordered {
                        value: &value,
                        sort: self.sort,
                        depth: self.depth + 1,
                    })?;
                }
                seq.end()
            }
            ValueKind::Number => {
                if !f64::try_from(self.value.clone())
                    .map_err(S::Error::custom)?
                    .is_finite()
                {
                    return Err(S::Error::custom("finite JSON required"));
                }
                self.value.serialize(serializer)
            }
            ValueKind::None | ValueKind::Bool | ValueKind::String => {
                self.value.serialize(serializer)
            }
            _ => Err(S::Error::custom("unsupported JSON value")),
        }
    }
}
struct PythonFormatter {
    ascii: bool,
    indent: Option<Vec<u8>>,
    item: Vec<u8>,
    key: Vec<u8>,
    containers: Vec<bool>,
}
impl PythonFormatter {
    fn newline<W: Write + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        if let Some(indent) = &self.indent {
            writer.write_all(b"\n")?;
            for _ in 0..self.containers.len() {
                writer.write_all(indent)?;
            }
        }
        Ok(())
    }
    fn begin<W: Write + ?Sized>(&mut self, writer: &mut W, byte: u8) -> io::Result<()> {
        self.containers.push(false);
        writer.write_all(&[byte])
    }
    fn end<W: Write + ?Sized>(&mut self, writer: &mut W, byte: u8) -> io::Result<()> {
        if self.containers.pop().unwrap_or(false) {
            self.newline(writer)?;
        }
        writer.write_all(&[byte])
    }
    fn element<W: Write + ?Sized>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
        if !first {
            writer.write_all(&self.item)?;
        }
        if let Some(last) = self.containers.last_mut() {
            *last = true;
        }
        self.newline(writer)
    }
}
impl Formatter for PythonFormatter {
    fn begin_array<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.begin(w, b'[')
    }
    fn end_array<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.end(w, b']')
    }
    fn begin_array_value<W: Write + ?Sized>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.element(w, first)
    }
    fn begin_object<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.begin(w, b'{')
    }
    fn end_object<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.end(w, b'}')
    }
    fn begin_object_key<W: Write + ?Sized>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.element(w, first)
    }
    fn begin_object_value<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(&self.key)
    }
    fn write_string_fragment<W: Write + ?Sized>(
        &mut self,
        w: &mut W,
        text: &str,
    ) -> io::Result<()> {
        if !self.ascii {
            return w.write_all(text.as_bytes());
        }
        for ch in text.chars() {
            if ch.is_ascii() && ch != '\u{7f}' {
                w.write_all(&[ch as u8])?;
            } else {
                let mut units = [0u16; 2];
                for code in ch.encode_utf16(&mut units) {
                    write!(w, "\\u{code:04x}")?;
                }
            }
        }
        Ok(())
    }
    fn write_f64<W: Write + ?Sized>(&mut self, w: &mut W, value: f64) -> io::Result<()> {
        w.write_all(python_float(value).as_bytes())
    }
}
/// Both runtimes choose the shortest round-tripping decimal. Python uses
/// scientific notation below exponent -4 and from 16, with a signed exponent.
fn python_float(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .into();
    }
    let raw = value.abs().to_string();
    let (mantissa, power) = raw
        .split_once(['e', 'E'])
        .map_or((raw.as_str(), 0), |(m, e)| {
            (m, e.parse::<i32>().expect("finite float exponent"))
        });
    let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let leading = digits
        .bytes()
        .position(|ch| ch != b'0')
        .expect("nonzero finite float");
    let significant = digits[leading..].trim_end_matches('0');
    let exponent = power + decimal - leading as i32 - 1;
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if !(-4..16).contains(&exponent) {
        let tail = if significant.len() > 1 {
            format!(".{}", &significant[1..])
        } else {
            String::new()
        };
        format!(
            "{sign}{}{tail}e{}{exponent_abs:02}",
            &significant[..1],
            if exponent < 0 { "-" } else { "+" },
            exponent_abs = exponent.unsigned_abs()
        )
    } else {
        let point = exponent + 1;
        let number = if point <= 0 {
            format!("0.{}{}", "0".repeat((-point) as usize), significant)
        } else if point as usize >= significant.len() {
            format!(
                "{}{}.0",
                significant,
                "0".repeat(point as usize - significant.len())
            )
        } else {
            format!(
                "{}.{}",
                &significant[..point as usize],
                &significant[point as usize..]
            )
        };
        format!("{sign}{number}")
    }
}
pub(crate) fn tojson(
    value: Value,
    positional_indent: Option<Value>,
    args: Kwargs,
) -> Result<Value, Error> {
    let ascii: bool = args.get::<Option<bool>>("ensure_ascii")?.unwrap_or(false);
    let sort: bool = args.get::<Option<bool>>("sort_keys")?.unwrap_or(false);
    let keyword_indent: Option<Value> = args.get("indent")?;
    if positional_indent.is_some() && keyword_indent.is_some() {
        return Err(Error::from(ErrorKind::TooManyArguments));
    }
    let indent = match positional_indent
        .or(keyword_indent)
        .filter(|value| !value.is_none())
    {
        None => None,
        Some(value) if value.kind() == ValueKind::String => {
            Some(value.as_str().unwrap().as_bytes().to_vec())
        }
        Some(value) => {
            let count = if value.kind() == ValueKind::Bool {
                usize::from(bool::try_from(value)?)
            } else {
                usize::try_from(value)?
            };
            if count > 1024 {
                return Err(Error::new(ErrorKind::InvalidOperation, "encoding_capacity"));
            }
            Some(vec![b' '; count])
        }
    };
    if indent.as_ref().is_some_and(|value| value.len() > 1024) {
        return Err(Error::new(ErrorKind::InvalidOperation, "encoding_capacity"));
    }
    let separators: Option<Vec<String>> = args.get("separators")?;
    args.assert_all_used()?;
    let (item, key) = match separators {
        Some(values) if values.len() == 2 && values.iter().all(|value| value.len() <= 1024) => {
            (values[0].as_bytes().to_vec(), values[1].as_bytes().to_vec())
        }
        Some(_) => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "unsupported separators",
            ));
        }
        None => (
            if indent.is_some() {
                b",".to_vec()
            } else {
                b", ".to_vec()
            },
            b": ".to_vec(),
        ),
    };
    let mut output = LimitedBuffer::new();
    let result = Ordered {
        value: &value,
        sort,
        depth: 0,
    }
    .serialize(&mut serde_json::Serializer::with_formatter(
        &mut output,
        PythonFormatter {
            ascii,
            indent,
            item,
            key,
            containers: Vec::new(),
        },
    ));
    if result.is_err() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            if output.exceeded {
                "encoding_capacity"
            } else {
                "unsupported JSON value"
            },
        ));
    }
    let text =
        String::from_utf8(output.bytes).map_err(|_| Error::from(ErrorKind::InvalidOperation))?;
    Ok(Value::from_safe_string(text))
}

/// Preserve Python scalar spelling for both interpolations and `|string`.
/// The serializer distinguishes integer-valued floats from integer inputs.
pub(crate) fn number_string(value: &Value) -> Result<Value, Error> {
    let mut output = LimitedBuffer::new();
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut output,
            PythonFormatter {
                ascii: false,
                indent: None,
                item: b", ".to_vec(),
                key: b": ".to_vec(),
                containers: Vec::new(),
            },
        ))
        .map_err(|_| Error::from(ErrorKind::InvalidOperation))?;
    String::from_utf8(output.bytes)
        .map(Value::from)
        .map_err(|_| Error::from(ErrorKind::InvalidOperation))
}
