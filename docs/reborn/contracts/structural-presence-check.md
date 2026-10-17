# Typed structural presence check

`validator_components::structural_presence_draft` packages one pure class-22
PythonCode component with the preloadable `check_structural_presence(inputs)`
export. It requires non-null `name`, `description` and `content` strings and
returns the strict `structural_presence` rule result (`rule_id`, `ok`, `errors`).
Only whitespace-stripped presence is checked. The function does not parse or
execute those strings, check class semantics, approve a component or write Q1.

Installation retains this shared draft under the stable
`validator:structural-presence` component identity through the immutable revision
store. Reseeding recovers its exact packaged revision while preserving a later
authored allocation head. It does not activate the draft or overwrite any of
the eight legacy validator bodies, checksums, references or routing entries.
Existing legacy compatibility code remains a separate migration concern.

Use this limited rule within a complete class-specific validator Recipe, with
typed input bindings, the other required rules, complete collection/aggregation
and trusted review evidence. The component alone cannot establish Q1, behavioral
qualification, class validation or an active validator route. Installation-owned
qualification and authored Q1/behavior/human-Q2 requirements apply to the actual
workflow and dependency combination before activation.
