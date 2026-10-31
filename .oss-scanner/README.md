# OSS Scanner setup

This directory keeps BrassClaw's scanner environment and threat model in the
project repository. It does not run a security scan by itself.

- `Dockerfile` builds the product and Monty worker and retains source and build
  tools for offline investigation. It is copied from the enrollment proposal;
  build and offline-test verification are still pending.
- `threat_model.md` describes trust boundaries, scope and report expectations.
- `project.yaml` is the enrollment configuration to copy to
  `projects/brassclaw/project.yaml` in `anthropics/oss-scanner` when switching
  enrollment to these repository-owned files.

## Switch the existing enrollment

The [enrollment PR](https://github.com/anthropics/oss-scanner/pull/1095) currently
contains its own Dockerfile and threat model. It continues to use those copies.

1. Publish this directory on BrassClaw's public `main` branch first.
2. Replace the enrollment's `projects/brassclaw/project.yaml` with this
   directory's `project.yaml`.
3. Remove the sibling Dockerfile and threat model from the enrollment PR.
   Each file must exist in exactly one of the two supported locations.
4. In the OSS Scanner checkout, run `python3 tools/validate.py`, then
   `python3 tools/check brassclaw` and verify the relevant tests offline.

The local configuration is a template for that migration; merely adding it to
BrassClaw does not update Anthropic's enrollment or start a scan. For the current
status and remaining work, see [mythbuster.md](../mythbuster.md). Private security
reports go to the contact in [SECURITY.md](../SECURITY.md).

## Direct build check

From the BrassClaw repository root on a Docker host:

```sh
docker build --platform linux/amd64 -f .oss-scanner/Dockerfile -t brassclaw-oss-scanner .
docker run --rm -it --network none brassclaw-oss-scanner bash
```

Use the `scanner` account for PostgreSQL tests, as described in the threat model.
Building the environment is distinct from running Anthropic's security scan.
