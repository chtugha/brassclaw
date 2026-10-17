# Prefix model artifacts

The Prefix settings model section retains configuration-only model specifications
against the existing PostgreSQL provider revision. **Import local config and
tokenizer files** attaches original files to the displayed specification. Select
`config.json`, `tokenizer.json` and `tokenizer_config.json`; optional files are
`special_tokens_map.json`, `added_tokens.json`, `vocab.json`, `merges.txt` and
`chat_template.jinja`. Model weights and programs are not accepted here.

Original bytes are stored once in a content-addressed artifact set. Several model
specifications and prefix generations can reference that set without copying the
large tokenizer into each generation. The binding records the authenticated
operator. Changing provider/model settings does not rewrite historical bindings.

An import establishes retained bytes and integrity. It does not establish that the
provider serves those exact weights/tokenizer/template, accept a model design,
qualify recurrent/attention cache transfer or activate a prefix. A malformed or
incompatible tokenizer can remain an unverified original; execution must validate
its supported adapter separately. Original model specifications retain their
configuration-only qualification.

## API

All routes use the existing instance-operator Bearer token and product facade.
Neither filesystem paths nor executable/compiler choices are request fields.

- `POST /api/webchat/v2/prefix-model-artifacts` accepts
  `{"specification":"<specification SHA-256>","artifact_set_bytes":"<JSON>"}`.
- `GET /api/webchat/v2/prefix-model-artifacts/{specification}/{artifact_set}`
  returns the exact canonical set and summary after checking every original file,
  the inventory and binding integrity.
- `GET /api/webchat/v2/prefix-model-designs` includes `artifact_sets` summaries
  under the existing model specifications. It does not send tokenizer contents in
  the ordinary catalogue response.

The browser upload format is:

```json
{
  "format": "brassclaw-prefix-model-artifact-import/1",
  "artifacts": {
    "config.json": {"encoding": "base64", "data": "<original file bytes>"},
    "tokenizer.json": {"encoding": "base64", "data": "<original file bytes>"},
    "tokenizer_config.json": {"encoding": "base64", "data": "<original file bytes>"}
  }
}
```

The retained format is `brassclaw-prefix-model-artifact-set/1`; each artifact also
contains its derived `sha256`. Uploading that retained format verifies the supplied
hashes. Both formats normalize to the same content identity for identical original
files, independent of property order or upload location. The HTTP envelope is
bounded to 64 MiB. The browser rejects a decoded set above 45 MiB before reading it;
no file is dropped to fit. An oversized set needs an explicit future transport
adapter, not clipping or a claim that the files were retained.

Repeated identical requests return the original binding. A conflicting binding
origin fails rather than replacing provenance. These records are immutable;
replacement means importing a new set and explicitly selecting it in a future
qualified build/design. Retention cannot supply its own approval or qualification
fields.

The default outer ingress ceiling is 64 MiB, matching this declared import route.
Chat, attachment and ordinary settings routes retain their smaller descriptor
limits. An explicit operator-configured smaller outer ceiling still applies and
can reject a large artifact import; it is never silently raised by this API.
