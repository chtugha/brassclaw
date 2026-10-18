//! Strict parser custody: Serde visitors retain indices, never owned subtrees.
//! One forest protects partial results through syntax, budget and trailing-data
//! failures. Reader/schema depth admission remains unchanged.
use std::{collections::BTreeMap, fmt};

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::{ContractError, ContractLimits, SizeBudget, error};

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
            .ok_or_else(|| E::custom("capacity"))?;
        let additional = self
            .visited
            .checked_sub(self.roots.len())
            .ok_or_else(|| E::custom("capacity"))?;
        self.roots
            .try_reserve(additional)
            .map_err(|_| E::custom("capacity"))?;
        let index = self.roots.len();
        // A pending container's placeholder counts as that container's node.
        // Closing it removes its child roots before filling this slot.
        self.roots.push(Value::Null);
        Ok(index)
    }
}

impl Drop for Forest {
    fn drop(&mut self) {
        // Owned descendants plus pending placeholders never exceed visited.
        // Reuse reserved capacity even after a late parse/budget failure.
        while let Some(value) = self.roots.pop() {
            match value {
                Value::Array(values) => self.roots.extend(values),
                Value::Object(values) => self.roots.extend(values.into_values()),
                _ => {}
            }
        }
    }
}

pub(super) fn parse(source: &str, limits: ContractLimits) -> Result<Value, ContractError> {
    if source.len() > limits.max_bytes {
        return Err(error("$", "JSON document byte bound exceeded"));
    }
    let mut budget = SizeBudget {
        limits,
        nodes: 0,
        bytes: 0,
    };
    let mut forest = Forest::default();
    let mut decoder = serde_json::Deserializer::from_str(source);
    Seed {
        budget: &mut budget,
        forest: &mut forest,
        depth: 0,
    }
    .deserialize(serde_stacker::Deserializer::new(&mut decoder))
    .map_err(|_| error("$", "invalid, duplicate-key or oversized JSON"))?;
    decoder
        .end()
        .map_err(|_| error("$", "trailing JSON data"))?;
    Ok(forest.roots.pop().expect("completed JSON root"))
}

struct Seed<'a> {
    budget: &'a mut SizeBudget,
    forest: &'a mut Forest,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.budget
            .enter(self.depth, 0)
            .map_err(serde::de::Error::custom)?;
        let index = self.forest.enter::<D::Error>()?;
        deserializer.deserialize_any(JsonVisitor {
            budget: self.budget,
            forest: self.forest,
            depth: self.depth,
            index,
        })
    }
}

struct JsonVisitor<'a> {
    budget: &'a mut SizeBudget,
    forest: &'a mut Forest,
    depth: usize,
    index: usize,
}

impl<'de> Visitor<'de> for JsonVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON data with unique keys")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Bool(value);
        Ok(())
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Number(value.into());
        Ok(())
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Number(value.into());
        Ok(())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<(), E> {
        let number = Number::from_f64(value).ok_or_else(|| E::custom("non-finite number"))?;
        self.forest.roots[self.index] = Value::Number(number);
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
        self.budget.bytes = self
            .budget
            .bytes
            .checked_add(value.len())
            .ok_or_else(|| E::custom("byte bound"))?;
        if self.budget.bytes > self.budget.limits.max_bytes {
            return Err(E::custom("byte bound"));
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| E::custom("capacity"))?;
        owned.push_str(value);
        self.forest.roots[self.index] = Value::String(owned);
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence
            .next_element_seed(Seed {
                budget: self.budget,
                forest: self.forest,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        let start = self.index + 1;
        let count = self.forest.roots.len() - start;
        let mut values = Vec::new();
        values
            .try_reserve(count)
            .map_err(|_| <A::Error as serde::de::Error>::custom("capacity"))?;
        values.extend(self.forest.roots.drain(start..));
        self.forest.roots[self.index] = Value::Array(values);
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget
                .enter(self.depth + 1, key.len())
                .map_err(serde::de::Error::custom)?;
            if keys.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate key"));
            }
            let index = self.forest.roots.len();
            map.next_value_seed(Seed {
                budget: self.budget,
                forest: self.forest,
                depth: self.depth + 1,
            })?;
            keys.insert(key, index);
        }
        let mut values = Map::new();
        // Parsing and duplicate/budget checks are complete. No fallible step
        // remains while the local map assembles the already guarded children.
        for (key, index) in keys {
            values.insert(key, std::mem::take(&mut self.forest.roots[index]));
        }
        self.forest.roots.truncate(self.index + 1);
        self.forest.roots[self.index] = Value::Object(values);
        Ok(())
    }
}
