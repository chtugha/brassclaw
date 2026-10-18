//! Immutable JSON at host boundaries. Cloning shares read-only data; final
//! cleanup uses reserved storage rather than the Rust call stack. This supplies
//! no resource policy: callers still validate their captured data/frame limits.
use std::{fmt, io, ops::Deref, sync::Arc};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};

/// An immutable, cheaply cloned JSON value with stack-safe final cleanup.
/// Mutable values produced inside a VM remain local to that task/invocation.
#[derive(Clone)]
pub struct JsonValue(Arc<Owned>);

struct Owned {
    value: Value,
    cleanup: Vec<Value>,
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.cleanup.push(std::mem::take(&mut self.value));
        dismantle(&mut self.cleanup);
    }
}

fn dismantle(pending: &mut Vec<Value>) {
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(values) => pending.extend(values.into_values()),
            _ => {}
        }
    }
}

impl JsonValue {
    /// Adopt an existing owned tree without copying its data. Like standard
    /// collection constructors, allocation failure aborts; it is not admission.
    pub fn new(value: Value) -> Self {
        let mut pending = vec![&value];
        let mut nodes = 0usize;
        while let Some(value) = pending.pop() {
            nodes = nodes
                .checked_add(1)
                .expect("JSON exceeds addressable memory");
            match value {
                Value::Array(values) => pending.extend(values),
                Value::Object(values) => pending.extend(values.values()),
                _ => {}
            }
        }
        drop(pending);
        Self(Arc::new(Owned {
            value,
            cleanup: Vec::with_capacity(nodes),
        }))
    }

    /// Borrow data without granting mutation of another handle's value.
    /// Raw Value cloning/formatting remains the caller's depth-safety concern;
    /// prefer cloning/serializing this handle.
    pub fn as_value(&self) -> &Value {
        &self.0.value
    }
}

impl From<Value> for JsonValue {
    fn from(value: Value) -> Self {
        Self::new(value)
    }
}
impl Deref for JsonValue {
    type Target = Value;
    fn deref(&self) -> &Value {
        self.as_value()
    }
}
impl Default for JsonValue {
    fn default() -> Self {
        Value::Null.into()
    }
}
impl fmt::Debug for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JsonValue(<private>)")
    }
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .value
            .serialize(serde_stacker::Serializer::new(serializer))
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Writer<'a, 'b>(&'a mut fmt::Formatter<'b>);
        impl io::Write for Writer<'_, '_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| io::Error::other("JSON formatting failed"))?;
                self.0
                    .write_str(text)
                    .map_err(|_| io::Error::other("JSON formatting failed"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        if formatter.alternate() {
            let mut serializer = serde_json::Serializer::pretty(Writer(formatter));
            self.serialize(&mut serializer).map_err(|_| fmt::Error)
        } else {
            let mut serializer = serde_json::Serializer::new(Writer(formatter));
            self.serialize(&mut serializer).map_err(|_| fmt::Error)
        }
    }
}

fn equal(left: &Value, right: &Value) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        match (left, right) {
            (Value::Null, Value::Null) => {}
            (Value::Bool(left), Value::Bool(right)) if left == right => {}
            (Value::Number(left), Value::Number(right)) if left == right => {}
            (Value::String(left), Value::String(right)) if left == right => {}
            (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
                pending.extend(left.iter().zip(right))
            }
            (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
                for (key, left) in left {
                    if let Some(right) = right.get(key) {
                        pending.push((left, right));
                    } else {
                        return false;
                    }
                }
            }
            _ => return false,
        }
    }
    true
}
impl PartialEq for JsonValue {
    fn eq(&self, right: &Self) -> bool {
        Arc::ptr_eq(&self.0, &right.0) || equal(self, right)
    }
}
impl Eq for JsonValue {}
impl PartialEq<Value> for JsonValue {
    fn eq(&self, right: &Value) -> bool {
        equal(self, right)
    }
}
impl PartialEq<JsonValue> for Value {
    fn eq(&self, right: &JsonValue) -> bool {
        equal(self, right)
    }
}
impl PartialEq<Value> for &JsonValue {
    fn eq(&self, right: &Value) -> bool {
        equal(self, right)
    }
}

#[derive(Default)]
struct Forest {
    roots: Vec<Value>,
    visited: usize,
}
impl Forest {
    fn enter<E: serde::de::Error>(&mut self) -> Result<usize, E> {
        self.visited = self
            .visited
            .checked_add(1)
            .ok_or_else(|| E::custom("JSON capacity exceeded"))?;
        let additional = self
            .visited
            .checked_sub(self.roots.len())
            .ok_or_else(|| E::custom("JSON capacity exceeded"))?;
        self.roots
            .try_reserve(additional)
            .map_err(|_| E::custom("JSON capacity exceeded"))?;
        let index = self.roots.len();
        self.roots.push(Value::Null);
        Ok(index)
    }
}
impl Drop for Forest {
    fn drop(&mut self) {
        dismantle(&mut self.roots);
    }
}

struct Seed<'a>(&'a mut Forest);
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        let index = self.0.enter::<D::Error>()?;
        deserializer.deserialize_any(JsonVisitor {
            forest: self.0,
            index,
        })
    }
}
struct JsonVisitor<'a> {
    forest: &'a mut Forest,
    index: usize,
}
impl<'de> Visitor<'de> for JsonVisitor<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON with unique object keys")
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Bool(value);
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::from(value);
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::from(value);
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Number(
            Number::from_f64(value).ok_or_else(|| E::custom("non-finite JSON number"))?,
        );
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| E::custom("JSON capacity exceeded"))?;
        owned.push_str(value);
        self.forest.roots[self.index] = Value::String(owned);
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(Seed(self.forest))?.is_some() {}
        let start = self.index + 1;
        let mut values = Vec::new();
        values
            .try_reserve(self.forest.roots.len() - start)
            .map_err(|_| <A::Error as serde::de::Error>::custom("JSON capacity exceeded"))?;
        values.extend(self.forest.roots.drain(start..));
        self.forest.roots[self.index] = Value::Array(values);
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = std::collections::BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate JSON key"));
            }
            let index = self.forest.roots.len();
            map.next_value_seed(Seed(self.forest))?;
            keys.insert(key, index);
        }
        let mut values = Map::new();
        for (key, index) in keys {
            values.insert(key, std::mem::take(&mut self.forest.roots[index]));
        }
        self.forest.roots.truncate(self.index + 1);
        self.forest.roots[self.index] = Value::Object(values);
        Ok(())
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut forest = Forest::default();
        Seed(&mut forest).deserialize(serde_stacker::Deserializer::new(deserializer))?;
        let value = forest.roots.pop().expect("completed JSON root");
        Ok(Self(Arc::new(Owned {
            value,
            cleanup: std::mem::take(&mut forest.roots),
        })))
    }
}
