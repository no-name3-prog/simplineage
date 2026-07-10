# Security Policy

## Supported versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

1. Prefer [GitHub Private Security Advisories](https://github.com/no-name3-prog/simplineage/security/advisories/new)
2. Or contact maintainers listed in [CODEOWNERS](.github/CODEOWNERS)

Include impact, reproduction steps, and affected versions when possible.

We aim to acknowledge reports within **72 hours**.

## Automated scanning

Every pull request runs **cargo-audit** and **cargo-deny** in GitHub Actions.
Dependabot opens weekly PRs for crates, Actions, and Docker base images.
You do **not** need these tools installed locally.
