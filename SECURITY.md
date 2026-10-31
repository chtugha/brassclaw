# Security Policy

## Reporting a vulnerability

Please report suspected security vulnerabilities in BrassClaw privately to
[chtugha@ganjacru.de](mailto:chtugha@ganjacru.de). This is the project's primary
security contact, including for Anthropic OSS Scanner reports.

Do not publish unpatched vulnerability details, exploit code or sensitive logs
in public GitHub issues or pull requests. If you are unsure whether an issue is
security-related, use the private contact first.

Please include, where available:

- The affected version or Git commit, operating system and relevant configuration.
- A description of the vulnerability, attacker prerequisites and expected impact.
- Reproduction steps or a minimal proof of concept using synthetic data.
- The affected files or code paths, and any suggested fix.
- Your preferred contact information and attribution preference.

Remove passwords, API keys, instance tokens, personal data and other secrets
from reports. Do not send production credentials. If encrypted communication is
needed, contact us first to arrange a suitable channel; no project OpenPGP key
is currently published here.

## Supported versions

Security investigation and fixes focus on the current `main` branch. Reports
affecting released versions are welcome; include the exact version so we can
assess applicability. Backports to older releases are considered individually;
there is no guaranteed long-term support or backport schedule.

## Handling and disclosure

We assess reports for reproducibility, impact and affected versions, and
coordinate remediation and disclosure with the reporter. Please keep details
private while we investigate and agree on an appropriate disclosure timeline.
Response and remediation times depend on maintainer availability and severity;
we do not promise a fixed response deadline.

Automated scanner findings are treated as reports requiring validation. A scan,
an accepted enrollment or a report without findings does not certify that
BrassClaw is free of vulnerabilities.

## Scope and safe reproduction

Relevant security boundaries include authentication, secrets, filesystem and
process sandboxing, network restrictions, live Tool policy, component approval,
and isolation between tasks and attempts. The instance-token operator is
intended to administer the instance; assess suspected issues against the actual
supported configuration and trust model. See
[the implementation status](docs/implementation-status.md) for implemented
boundaries and known migration work.

Reproduce issues only in systems you own or are authorized to test, using
isolated test instances and synthetic secrets. Avoid accessing others' data,
disrupting live services or testing third-party infrastructure.
