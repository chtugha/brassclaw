use crate::{EncodingError, MAX_RENDER_BYTES, TemplateFailure, context, python_json};
use indexmap::IndexMap;
use minijinja::{
    AutoEscape, Environment, Error, ErrorKind, Value, syntax::SyntaxConfig, value::ValueKind,
};
use std::io::{self, Write};

pub(crate) struct LimitedBuffer {
    pub(crate) bytes: Vec<u8>,
    pub(crate) exceeded: bool,
}
impl LimitedBuffer {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            exceeded: false,
        }
    }
}
impl Write for LimitedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > MAX_RENDER_BYTES)
        {
            self.exceeded = true;
            return Err(io::Error::other("encoding_capacity"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) struct Renderer<'a> {
    env: Environment<'a>,
    defaults: IndexMap<String, Value>,
}
impl<'a> Renderer<'a> {
    pub(crate) fn new(template: &'a str, config: Value) -> Result<Self, EncodingError> {
        if config.kind() != ValueKind::Map {
            return Err(EncodingError::InvalidInput);
        }
        let mut env = Environment::new();
        env.set_auto_escape_callback(|_| AutoEscape::None);
        env.set_recursion_limit(64);
        env.set_fuel(Some(5_000_000));
        env.set_syntax(
            SyntaxConfig::builder()
                .trim_blocks(true)
                .lstrip_blocks(true)
                .build()
                .map_err(|error| template_failure(&error))?,
        );
        env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        // MiniJinja permits iterating None as empty, but Python/Jinja's
        // `is iterable` test is false for None. Prefix wrappers branch on it.
        env.add_test("iterable", |value: &Value| {
            !value.is_none() && value.try_iter().is_ok()
        });
        env.add_filter("tojson", python_json::tojson);
        env.add_filter("string", |state: &minijinja::State, value: &Value| {
            if value.kind() == ValueKind::Number {
                python_json::number_string(value)
            } else {
                minijinja::filters::string(state, value)
            }
        });
        env.set_formatter(|output, state, value| {
            if value.kind() == ValueKind::Number {
                minijinja::escape_formatter(output, state, &python_json::number_string(value)?)
            } else {
                minijinja::escape_formatter(output, state, value)
            }
        });
        env.add_function(
            "raise_exception",
            |_message: Value| -> Result<Value, Error> {
                Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "model template rejected input",
                ))
            },
        );
        env.add_function("strftime_now", |_format: Value| -> Result<Value, Error> {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                "nondeterministic model template",
            ))
        });
        // No filesystem loader, network helpers or caller-supplied functions.
        env.add_template("model", template)
            .map_err(|error| template_failure(&error))?;
        let mut defaults = IndexMap::new();
        for name in [
            "bos_token",
            "eos_token",
            "unk_token",
            "sep_token",
            "pad_token",
            "cls_token",
            "mask_token",
        ] {
            let value = config
                .get_item(&Value::from(name))
                .map_err(|_| EncodingError::InvalidInput)?;
            if value.is_none() || value.is_undefined() {
                continue;
            }
            let value = if value.kind() == ValueKind::Map {
                value
                    .get_item(&Value::from("content"))
                    .map_err(|_| EncodingError::InvalidInput)?
            } else {
                value
            };
            if value.kind() != ValueKind::String {
                return Err(EncodingError::InvalidInput);
            }
            defaults.insert(name.to_owned(), value);
        }
        Ok(Self { env, defaults })
    }
    pub(crate) fn render(&self, json: &str) -> Result<String, EncodingError> {
        let context = context::parse(json)?;
        if context.kind() != ValueKind::Map {
            return Err(EncodingError::InvalidInput);
        }
        let mut values = self.defaults.clone();
        for key in context
            .try_iter()
            .map_err(|_| EncodingError::InvalidInput)?
        {
            let name = key.as_str().ok_or(EncodingError::InvalidInput)?;
            if matches!(
                name,
                "raise_exception" | "strftime_now" | "namespace" | "dict" | "range"
            ) {
                return Err(EncodingError::InvalidInput);
            }
            values.insert(
                name.to_owned(),
                context
                    .get_item(&key)
                    .map_err(|_| EncodingError::InvalidInput)?,
            );
        }
        let mut output = LimitedBuffer::new();
        if let Err(error) = self
            .env
            .get_template("model")
            .map_err(|error| template_failure(&error))?
            .render_captured_to(
                Value::from_object(
                    values
                        .into_iter()
                        .map(|(key, value)| (Value::from(key), value))
                        .collect::<IndexMap<Value, Value>>(),
                ),
                &mut output,
            )
        {
            return Err(
                if output.exceeded
                    || error.kind() == ErrorKind::OutOfFuel
                    || error.detail() == Some("encoding_capacity")
                {
                    EncodingError::Capacity
                } else {
                    template_failure(&error)
                },
            );
        }
        String::from_utf8(output.bytes).map_err(|_| EncodingError::Template {
            kind: TemplateFailure::Other,
        })
    }
}

fn template_failure(error: &Error) -> EncodingError {
    let kind = match error.kind() {
        ErrorKind::OutOfFuel => return EncodingError::Capacity,
        ErrorKind::SyntaxError => TemplateFailure::Syntax,
        ErrorKind::TemplateNotFound => TemplateFailure::MissingTemplate,
        ErrorKind::InvalidOperation => TemplateFailure::InvalidOperation,
        ErrorKind::TooManyArguments | ErrorKind::MissingArgument => TemplateFailure::Arguments,
        ErrorKind::UnknownFilter => TemplateFailure::UnknownFilter,
        ErrorKind::UnknownTest => TemplateFailure::UnknownTest,
        ErrorKind::UnknownFunction => TemplateFailure::UnknownFunction,
        ErrorKind::UnknownMethod => TemplateFailure::UnknownMethod,
        ErrorKind::BadEscape => TemplateFailure::Escape,
        ErrorKind::UndefinedError => TemplateFailure::Undefined,
        ErrorKind::BadSerialization | ErrorKind::CannotDeserialize => {
            TemplateFailure::Serialization
        }
        ErrorKind::CannotUnpack => TemplateFailure::Unpack,
        _ => TemplateFailure::Other,
    };
    EncodingError::Template { kind }
}
