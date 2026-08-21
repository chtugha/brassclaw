---
paths:
  - "crates/**"
  - "tests/**"
---
# Typed Internals — No Stringly-Typed Values Inside the System

**Every domain value gets a specialized type.** Raw `String` is a boundary
format — accepted from user input, JSON/HTTP payloads, the database,
and untrusted external APIs — and converted to a domain type at the
earliest opportunity. Everything flowing between internal modules must
carry a type that makes misuse a compile error.

- **Identifiers** → newtypes (`TenantId`, `UserId`, `AgentId`, `ProjectId`, `ThreadId`). Never `String`, `&str`, or `uuid::Uuid` alone.
- **Fixed small sets** → enums with `#[serde(rename_all = "snake_case")]`
  or explicit `#[serde(rename = "...")]`. Never compare strings like
  `status == "in_progress"`.
- **Units, shapes, modes** → enums (`RuntimeProfile`, `ValidationStatus`,
  `IntentClass`). Never booleans-plus-magic-strings.

Two values with the same shape but different meanings must be
different types. The compiler is the only durable enforcement —
comments, naming, and code review are not.

## Why

Identity confusion has shipped multiple times in recent history. Same shape every time: a string-typed value passes through more than one layer, one layer treats it as one meaning, another as a different meaning, and the compiler has nothing to say. Newtypes would have turned each into a compile error.

## Identity types

The scope identity tuple `(tenant_id, user_id, agent_id, project_id)` appears throughout the codebase. Never re-derive one component from another by string manipulation. Never pass bare `String`s where typed identity is expected.

## Canonical newtype template

New newtypes use this single shape. Validation happens on the wire
(`try_from`) and at explicit construction (`::new`), both routed
through a shared `validate(&str)`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct MyId(String);

impl MyId {
    fn validate(s: &str) -> Result<(), MyIdError> { /* ... */ }

    pub fn new(raw: impl Into<String>) -> Result<Self, MyIdError> {
        let s = raw.into();
        Self::validate(&s)?;
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str { &self.0 }
    pub fn into_inner(self) -> String { self.0 }
}

impl TryFrom<String> for MyId {
    type Error = MyIdError;
    fn try_from(value: String) -> Result<Self, MyIdError> {
        Self::validate(&value)?;
        Ok(Self(value))
    }
}

impl AsRef<str> for MyId {
    fn as_ref(&self) -> &str { &self.0 }
}

impl fmt::Display for MyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<MyId> for String {
    fn from(id: MyId) -> Self { id.0 }
}
// Deliberately no `From<String>` / `From<&str>` — infallible
// conversion would silently bypass validation.
// Deliberately no `Deref<Target = str>` — auto-deref would let
// `&id` silently coerce to `&str`, which is the implicit-conversion
// pattern this rule exists to prevent.
```

Rules baked into the template:

- `#[serde(try_from = "String")]` — wire validation matches
  construction; do not use `#[serde(transparent)]` on a newly added
  validated newtype.
- Shared `validate(&str)` — one source of truth for the invariant.
- `impl Into<String>` on `new` — avoids a clone for owned-`String`
  callers; still accepts `&str`.
- Explicit `as_str()` / `as_ref()` / `into_inner()` — every boundary
  crossing is visible in the source.
- Match-on-string-literals means the type should be an enum. Fix the type.
- Don't return `String` from an internal function — return the newtype.

## Byte-length vs. character-length

A validator using `s.len()` measures bytes. If the error message says
"N characters", switch to `s.chars().count()`. Pick one and match the
message.

## Wire-stable enums

Enums serialized over the network or persisted to the DB are part of
the public contract.

Derive `Serialize` + `Deserialize` with
`#[serde(rename_all = "snake_case")]`. Add enum helper methods for
wire/UI rendering — never `format!("{:?}", ...)`.

**Migrations from `String` must preserve every historical value.**
When replacing a stringly-typed wire field with an enum, add
`#[serde(alias = "...")]` for every value any running producer still
emits. Grep the tree; check staging/production logs. Add a round-trip
deserialization test with raw legacy JSON.

## Wire-contract field naming

A boolean or enum exposed to the web UI has exactly one canonical
snake_case name on the wire and one canonical JS accessor. Reading the
same value from ad-hoc data fields inside a surface file is a bug —
it will diverge. Delete duplicate fields in response structs.

## Applies to

`crates/**`, `tests/**`. Any code inside the BrassClaw workspace. The rule doesn't apply to wire payloads (which are `String` by virtue of JSON), log lines, or error messages — those *are* the boundary.
