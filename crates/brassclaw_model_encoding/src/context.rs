//! Bounded ordered JSON decoding. Tool JSON key order is part of chat-template
//! output; parsing through an unordered/sorted intermediate would change tokens.
use crate::{EncodingError, MAX_INPUT_BYTES};
use indexmap::IndexMap;
use minijinja::Value;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;
struct Budget {
    nodes: usize,
}
struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.budget.nodes = self
            .budget
            .nodes
            .checked_add(1)
            .ok_or_else(|| de::Error::custom("context capacity"))?;
        if self.depth > 64 || self.budget.nodes > 2_000_000 {
            return Err(de::Error::custom("context capacity"));
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded ordered JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        if !value.is_finite() {
            return Err(E::custom("finite JSON required"));
        }
        Ok(Value::from(value))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::from(()))
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::from(()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = access.next_element_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::from(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = IndexMap::<String, Value>::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate context key"));
            }
            let value = access.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::from_object(
            values
                .into_iter()
                .map(|(key, value)| (Value::from(key), value))
                .collect::<IndexMap<Value, Value>>(),
        ))
    }
}
pub(crate) fn parse(bytes: &str) -> Result<Value, EncodingError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(EncodingError::Capacity);
    }
    let mut budget = Budget { nodes: 0 };
    let mut deserializer = serde_json::Deserializer::from_str(bytes);
    let value = Seed {
        budget: &mut budget,
        depth: 0,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| EncodingError::InvalidInput)?;
    deserializer
        .end()
        .map_err(|_| EncodingError::InvalidInput)?;
    Ok(value)
}
