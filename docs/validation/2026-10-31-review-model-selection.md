# Retained validator model-input preparation

BUILD-014 fixes the existing preparation reader rejecting the compiler's valid
`model_input_binding` field. Engine and the reader now use the existing
`RecipeModelInput` serializer. The reader compares the envelope against the
actual immutable Recipe document and selected variant using the same parser as
IBS. Historical absence remains different from an explicit Default declaration.
Unknown fields and changed profiles, purposes or formats remain errors.

The [receipt](2026-10-31-review-model-selection.json), SHA256
`347be24f5ae84c02360a84769d44d50ff6cbaf45a7b951fe6153248dd299ec6e`,
records five sources,7996 named captured input entries, exact test/worker
images and logs. Run1 passes compilation, nine distinct tests/zero ignored,
format and connected/default strict Clippy. Original runner96588 and Cargo
processes are absent; the common lock was independently acquired before final
acceptance. Root retains sole Cargo ownership.

The new real PostgreSQL/IBS/worker-parser/restoration case covers absent bindings,
explicit Default, two selected profiles, a map for an unselected sibling and a
capture-only PrefixFree declaration. It rejects rewritten nested bindings with
correct recomputed checksums, retains original bytes on repeated writes and
restores every original selection after later Recipe successors. It also checks
zero review-evidence, association-approval, task-admission and Tool-invocation
rows. Existing bound structural observers, native Default/Selected SDK consumers,
capture-without-grant and shared parser tests provide the other eight cases.

This is preparation/data-integrity acceptance. It creates no Q1, human Q2,
general Pure approval, PrefixFree qualification, dispatch permission or activated
workflow. Existing SDK cases use a local recording endpoint; structural observers
remain partial checks. Full I1/I2, original Use/ModelSetup, general review routes,
recovery, platform and complete-plan/release requirements remain open.

The newer authored-own-function/system-seed documentation target is a separate
following package. This receipt preserves its original source inputs and does
not certify that successor's embedded document bytes or runtime implementation.
The named input inventory does not enumerate every compiler-embedded document.
BUILD-015 explicitly adds literal `include_str!`/`include_bytes!`/`include!` file
discovery and reruns the affected native packaging/reference consumers. That
expanded capture does not retrospectively broaden this original receipt.
