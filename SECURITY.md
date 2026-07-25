# Security Policy

## Reporting a vulnerability

If you discover a vulnerability in any StreamFi contract, please **do not** open a public GitHub issue. Instead, email the maintainers with details:

- **security@streamfi.media** (or open a private security advisory on GitHub)

Include:

- A description of the issue
- Steps to reproduce
- Any proof-of-concept code or transactions
- Your assessment of the impact
- Your contact info for follow-up

We aim to acknowledge reports within 72 hours and to publish a fix or coordinated disclosure within 30 days for critical issues.

## Supported versions

Only the `main` branch and the latest released contract WASM hashes are supported. Older deployments run at the discretion of whoever deployed them.

## Scope

In scope:
- Contracts under `contracts/*`
- Deployment scripts if/when added
- CI configuration that affects release artifacts

Out of scope:
- Third-party dependencies (report those upstream)
- Issues in the StreamFi frontend (see [streamfi-frontend](https://github.com/StreamFi-x/streamfi-frontend))
- Social engineering, physical attacks, DoS against public infra
