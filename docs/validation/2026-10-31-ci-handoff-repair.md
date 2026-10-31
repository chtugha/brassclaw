# BUILD026 — claimed profile identity and Docker source repairs

The [machine receipt](2026-10-31-ci-handoff-repair.json) retains both original
serial runs, exact source/image identities and logs. SHA-256: `3281e06ab8213877d527e3b2a2648beb59c964a32a6ec106fd0ce7dade6dcfeb`.

Persistence and the host already represented `interactive_default` as `default`.
The strict claimed Monty handoff compared the unprojected ID and rejected valid
claims before driver dispatch. One shared projection now serves all three owners.
Full profile, version, actor, input, run and attempt checks remain. No stop
acknowledgement is invented and no Rust workflow fallback is added.

All105 original CI-profile host cases pass, including the four GitHub failures.
The102-case default configuration,22 runner regressions, Turns core/host contracts,
complete CI-profile Reborn library, actual global Monty/PostgreSQL retained claim
consumer,32 architecture cases and connected strict Clippy also pass. No ignored
cases. The unchanged worker is verified against its actual dependency closure.

Docker now carries all path-patched vendor and embedded script inputs. The pinned
Chef upgrade is accompanied by checked generated-manifest migration: only redundant
per-target editions are removed, with full parsed TOML equivalence; differing
editions fail visibly. Five pure transformer cases pass. The Docker-only successor
removes `panic=abort`, preserving the package's unwinding needed by existing host
and task failure boundaries. Unchanged native results are reused.

Actual Docker compilation remains pending on GitHub. These are local macOS
results, not full CI/platform/deployment, component approval or full-plan completion.
The failed69024bee workflow remains an unchanged historical observation. All
remaining K3, Validator/Pure/S, Prefix/Serving, recovery and release gates remain.
