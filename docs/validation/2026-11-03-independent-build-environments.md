# Independent build-environment coordination

Date: 2026-11-03. Scope: development tooling, not BrassClaw product execution.
See the [operating contract](../build-environments.md) and
[exact source/check record](2026-11-03-independent-build-environments.json).

Native Cargo and Modal now have separate host settings and OS locks. The remote
lock covers unsettled-job preflight, source upload/readback, submission and
terminal collection without the former sync/submission lock gap. Unresolved
remote jobs continue to fence remote writes after a controller disconnect; they
do not block native builds. Local jobs inherit the canonical configured target;
no second editable checkout or native target is created. Remote CPU/RAM requests,
limits and Cargo jobs are independent. Checked-in defaults retain the qualified
eight-core settings; no increased resource allocation was launched.

27 distinct local checks passed: 14 settings/lock/controller cases in the
connected batch, two subsequently added focused cases and the 11 existing
source/worker cases. Earlier intermediate passes are not additional cases.
The added status test initially had an incorrect fixture directory, which was
corrected before its passing rerun. It confirms that original-volume recovery
cannot be redirected by a later current CLI option. Real subprocesses verified
cross-environment lock overlap and same-environment exclusion. Controller tests
mock the SDK transport; they are not remote execution receipts.

Settings/help inspection and whitespace checks passed. No Cargo command, image
build, worker provisioning or live Modal submission ran for these changes.
The existing qualified image and embedded compiler-worker code are unchanged.
The next necessary, independently authorized build can supply live acceptance
of the modified host controller; no paid verification-only run was started.

The AGENTS/CLAUDE policy edits are embedded product bytes. Existing compiled
artifacts and exact seed/prefix receipts retain their original scope; this
tooling validation does not qualify or activate new component bytes. The native
lock implementation was exercised on macOS, without claiming new Windows/Linux
runtime acceptance. Existing pending validator/Auth/Bob work is unchanged.
