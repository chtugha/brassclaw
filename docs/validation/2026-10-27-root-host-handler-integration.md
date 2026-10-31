# Root host integration and HTTP response-file safety

Status: **scoped acceptance passed**. This closes the four remaining file/HTTP
failures in the selected-lease package's Root matrix, not K3 or the full plans.

## Implemented boundary

PostgreSQL-free local host composition no longer attaches an unbound acknowledged
primitive-settings source. These contained host-only consumers use canonical
defaults. Actual PostgreSQL runtime composition still attaches that source and
fails closed until the existing global Monty startup/bootstrap/live binding
acknowledges its settings. There is no application runtime or Rust-loop fallback.

The Root HTTP recorder now delegates to the actual staged-policy, sanitization
and scoped response-body storage pipeline. Only external network responses are
scripted. HTTP-save checks the actual file bytes and exact saved-body receipt;
the handler's rejection of an absent/mismatching save receipt remains unchanged.
Local mount descriptors consume the backend's actual capabilities. LocalFilesystem
advertises its byte operations as well as opaque writes, without claiming record
body CAS or other unsupported record operations.

The sole production consumer of `write_bytes_with_mount_view` is the HTTP body
store. This scoped byte-writing adapter now calls the existing atomic
`put_opaque`, so a download cannot replace a structured entry. Trusted record
owners retain their explicit `put` operation. The HTTP contract proves a
post-transport save failure preserves the complete structured entry/version;
the native boundary case exercises actual body-store success and rejection on
both memory and PostgreSQL, with the captured invocation grant and no permissive
resolver. No source substitution, authority relaxation or validation bypass is
introduced.

The connected Root fixtures also retain the actual native transcript/turn
admission participant and distinct direct-user/adapter/route identities. Explicit
children retain their transcript's owner in the child TurnScope. These remain
documented historical planned-loop parity fixtures. Production acceptance uses
the existing global worker callers; these tests do not install a fallback driver.

## Actual evidence

The terminal R6 receipt is
`target/validation/selected-lease-20261027/root-handler-repair-6/accepted.json`,
SHA256 `0ad0784d6d2f3149547883628fc104c20a232318476670c592b3d85b0c0a136c`.
All code was compiled from the common checkout with the inherited canonical
Cargo target and serial background-screen execution.

| Actual passing cases | Count | Execution/provenance |
| --- | ---: | --- |
| Root integration matrix | 36 | All 23 programs executed in R5, including the four formerly failing cases and all five child cases. |
| Filesystem catalog | 6 | R5 actual local backend operations and advertised capabilities. |
| Host runtime units | 221 | R5; three separately named existing Docker cases were ignored and are **not** passing evidence. |
| HTTP egress contract | 38 | R5, including actual save bytes and structured-entry preservation. |
| Standalone local memory | 1 | Fresh R6 native factory caller without a PG/Monty settings owner. |
| Native JSON settings | 1 | Fresh R6 real global Monty/PostgreSQL startup, unbound-source rejection, acknowledged live settings and retained policy. |
| Native coding settings | 1 | Fresh R6 real global Monty/PostgreSQL caller with live WebUI limits and retained primitives. |
| Native record boundary | 1 | Fresh R6 actual memory/PostgreSQL coding and HTTP-body-store operations. |

Total: **305 distinct passing cases**. R6 explicitly reuses R5's 301 executed
successes only after verifying complete source membership/bytes and every actual
retained executable hash. Four native/local cases are freshly executed in R6.
The receipt records that distinction and the original evidence paths; these are
not 305 fresh R6 executions. The frozen source predates this report/status update.

Strict Root23, Composition tests, filesystem catalog and host HTTP/unit Clippy
passed with `-D warnings`. Composition default-library Clippy, workspace formatting
and whitespace passed. All four native selectors ran one test each, zero ignored.
The actual Composition image SHA256 is
`8a25bce0b99f4c852462409bee650b44a0b7860329c496640f063fd5fdc16d76`;
the actual separately built Monty worker SHA256 is
`a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.

Earlier diagnostic failures and interrupted source-review runs remain unchanged.
R5's native build command incorrectly selected the worker in the Composition
package and failed before compilation; R6 uses its actual `brassclaw_monty_host`
owner. This command failure neither invalidates the preceding exact-source
successes nor supplies native acceptance. No validation process remained at the
terminal check.

## Remaining work

Persistent exact requirement/account publication, deliberate sharing and original
task-family credential/material selection remain K3 work. The Prefix owner keeps
its design/generation/serving cutover. The remaining K3–K7, validator/component
migrations, durable recovery and platform/deployment acceptance remain open.
Docker is not installed on this validation host; its three unexecuted cases remain
explicit limitations, not a complete-platform claim.
