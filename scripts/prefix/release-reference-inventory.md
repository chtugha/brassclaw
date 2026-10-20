# Release reference inventory

`release-reference-inventory.json` is input to the existing offline
`base-promt-compiler.py --source-root ... --inventory ...` adapter. It is not
another compiler, provider catalogue, approval store or runtime discovery path.
The installed source owner must retain the same exact source package before
normal Generate can claim this coverage; the existing core-only capture does
not acquire it merely because this file exists.

The inventory covers root Markdown (except private `LOCAL_TEST_ENV.md`), all
Markdown under `docs/reborn/`, and crate `AGENTS.md`/`CLAUDE.md` guides. It also
includes the packaged Prefix-Maker/Sempai authoring references, the core design
record and this inventory/procedure. Each path is a logical release path. Original
bytes and calculated hashes belong to the package produced from a frozen source
root; no checkout paths or manually fixed content hashes are used as identity.

`included` means selected reference content, not activated runtime support.
The four component guides and final-v3 contracts take precedence over older
examples. A target requirement or implementation plan is not evidence of
implemented enforcement. The core guides remain complete. Additional material
without a current source/behavior review is explicitly `development-only` and
is archived in full, not silently promoted into the model view. A known
superseded document is `superseded`; neither disposition deletes its original.
This is a conservative initial selection, not a claim that every subsystem
contract has been reconciled with final v3.

Sources outside these declared roots, other release documentation, the complete
eligible private component library, domain sources and upstream Monty documentation
remain separate coverage work. In particular, do not confuse host SDK examples,
preload definitions, invocation bodies and Recipe authoring when incorporating
Monty material. The dedicated Sempai reference retains those distinctions.

Refresh this inventory through review when documents or release versions change.
A new file requires an explicit disposition/reason before it is included. Use a
frozen source tree with the catalogue export from that same tested release. A
successful package build proves integrity and coverage of the declared inventory,
not content quality, target capacity, cache reuse or serving readiness. Preserve
the source package and failed builds when complete mandatory content cannot fit.
