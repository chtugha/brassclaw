# Extract reusable v3 components and Recipes

Follow [recipe.md](../../../recipe.md) and root AGENTS.md. This instruction
replaces the legacy MemoryDoc/name-based extraction pattern. It does not add
new host APIs or establish that the component proposal path is wired here.

From a successful task, propose reusable components and a Recipe that tells
the orchestrator how to fulfill that task's goal:

- Rust Tool: primitive; reuse existing capabilities first.
- ToolSkill: one Rust-side IBS binding descriptor.
- Skill: prose explaining one Tool usage plus associated executable PythonCode.
- PythonCode: a small reusable executable operation with declared inputs/results.
- Recipe: ordered component steps, variants, intent examples and typed bindings.
- ExtensionCatalogue: domain overview when needed, not a multi-tool Skill.

Inspect the exact source task, Tool signatures, arguments, results and failure
behavior. Reuse existing component UUIDs. Do not extract secrets, claim tokens,
user-specific payloads or arbitrary runtime output into executable source.
Write the Recipe design first, then fill missing PythonCode, ToolSkill and Skill
associations. Supply at least ten intent examples and explicit negative cases.

Each Recipe component step has exactly one UUID. Internal PythonCode includes
are allowed, but must retain reusable contracts, avoid cycles/conflicts and
respect one independent Tool call per body (the direct dependent-chain exception
remains). Rust binding precedes Python execution; binding executes nothing.
Tier 0 executes class-22 code without interpreting prose. Shell/spawn-subagent
Recipes and creative composition require Tier 1.

Declare semantic inputs, types, validation/default rules, per-step local bindings
and result handoffs under recipe.md's target convention. Runtime values remain
data. IBS resolves the newest activated approved versions consistently and pins
all selected/nested versions in BuildInstruction; Recipe references omit version
numbers. Changes create new immutable versions; do not overwrite approved rows.
Existing tasks retain their versions across waits and resume.

Submit proposals through the supported component-library proposal/validation
path available in the current execution context. Never use legacy memory_write
or guessed host calls as substitutes. If the required path is unavailable,
return a structured proposal identifying the missing integration; do not claim
it was stored or activated. Authored versions require Q1 and human Q2. A new
Rust Tool is justified only by a missing primitive; source generation/build is
an explicit Tier-1 workflow, and compilation is not approval or registration.

Report reused/new UUIDs, proposed associations, Recipe/variant steps and inputs,
validation status, behavioral evidence and remaining runtime gaps. Never report
successful activation from a draft, syntax check or compilation alone.
