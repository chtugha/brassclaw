# CI test context — bounded Bob1 follow-up

Scope: repair the real `3a670a5ac` panic-check failure without changing component
approval, the production qualification owner or test identities. The observed
helper belongs to a `#[cfg(test)]` module declared in another file. Filename
guesses do not establish that Rust condition. The existing checker also wrongly
accepts `cfg(not(test))`, `cfg(any(test, feature = "x"))` and an unguarded module
merely named `tests` as test-only contexts.

## Steps

1. Preserve the production `bootstrap_validator_approval.rs` bytes and existing
   test module path. Add an inner `#![cfg(test)]` to the actual acceptance file;
   preserve the remaining file bytes, registered tests and native assertions.
   The exploratory path relocation is superseded, not an accepted source layout.
2. In `scripts/check_no_panics.py`, evaluate the documented single-line `cfg`
   predicate grammar conservatively with `test` disabled. Unknown configuration
   options may be true or false. Support `all`, `any`, `not`, boolean literals,
   ordinary key/value strings and optional list trailing commas. Unknown or
   malformed syntax must not create a test-only exemption. Keep this a static
   source check, not a new Rust compiler or an approval verifier.
3. Propagate a proven inner attribute through its actual containing file/block;
   outer attributes apply to their next item. Preserve real test attributes and
   test-directory handling. A module name alone is insufficient. Do not add
   `allow`, a file allowlist or safety-comment exemptions to fix this finding.
4. Exercise the real checker with positive and negative predicate/context cases,
   including strings/comments, nested predicates, adjacent production items and
   malformed input. Verify the former actual helper now has test context while
   unguarded production panic calls remain detectable. Run the changed-source
   check, formatting/whitespace and affected compilation where required; reuse
   native behavior evidence because its bodies/assertions are unchanged.
5. Record exact source applicability. The successful exploratory relocation lint
   is historical evidence for that source, not final-source acceptance. Keep the
   old GitHub failure visible. Update the existing validation receipt/changelog,
   commit/push reviewed changes, then resume the remaining Bob work.

The independent Encoding fixture correction remains in the existing executable
start follow-up: deadline before exec requires the actual start cause and no
child cleanup; deadline after start requires actual exit/reap custody. Retain its
actual offline oracle and model originals, and run the failed target itself.

Grammar authority: [Rust conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html)
and [test attributes](https://doc.rust-lang.org/reference/attributes/testing.html).
Unsupported multiline/raw-string attribute forms remain conservatively scanned;
they do not receive exemptions merely because they contain the word `test`.
