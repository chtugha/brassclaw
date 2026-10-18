# BrassClaw chat-template compatibility

Upstream MiniJinja 3.0.1 source and Apache-2.0 license are retained here.
`BRASSCLAW_UPSTREAM.json` records the registry archive and original file hashes;
`BRASSCLAW_PATCH.diff` records the source change. The extra Cargo feature is
`brassclaw_eager_concat`; only the model encoder enables it.

With the feature enabled, sequence concatenation uses the existing checked eager
materializer, preserving indexable/sliceable Jinja list results. Without it the
upstream lazy behavior remains. This is needed for the existing Ornith injection
wrapper's `messages = [system] + messages[1:]` followed by indexing and reverse
slicing. Changing the template to hide the incompatibility would make its measured
identity differ from the actual supplied template.

This runs only in the contained encoding utility worker with its allocation and
deadline bounds. It does not add template loaders, host callbacks or Python code
execution. The bounded ordered context decoder and independent Hugging Face
oracles remain required. Other MiniJinja/Jinja differences are not implicitly
qualified by this change. Retain the exact worker/native identities and requalify
supported templates on any dependency or compatibility change.

The encoder environment separately supplies Python JSON rendering and corrects the
`iterable` test for None. Upstream treats None as an empty iterable; Python's Jinja
test returns false. The distinction matters for nullable system-message content in
the same injection wrapper. These environment adapters are reviewed/tested in the
encoder crate rather than additional changes to the upstream engine.
