# Security Policy

## Supported versions

Security fixes target the latest published 0.1.x release and the current `main`
branch. Older 0.1.x builds may be asked to upgrade before a fix is backported.
Unreleased 0.2 work is supported only on `main`.

## Report a vulnerability

Do not open a public issue for suspected vulnerabilities. Email
`info@turaai.net` with:

- affected version or commit and operating system;
- affected component and configuration;
- reproduction steps or proof of concept;
- expected impact and whether exploitation has been observed;
- suggested mitigation, if known;
- a safe way to contact you.

Do not include live credentials, private session data, or destructive payloads.
Use minimal test data and redact secrets from logs.

We aim to acknowledge a report within 5 business days, provide an initial
assessment within 10 business days, and coordinate remediation and disclosure
based on severity and fix readiness. These are targets, not guarantees.

## Research guidelines

- Test only systems and data you own or are authorized to use.
- Avoid privacy violations, service disruption, persistence, lateral movement,
  and data destruction.
- Stop when you encounter private data or evidence of active exploitation.
- Do not use denial-of-service, social engineering, or credential attacks.
- Allow reasonable time for remediation before disclosure.

Good-faith research following this policy will not be intentionally pursued by
the project, but this policy cannot authorize testing of third-party providers or
systems.

## Secrets

Never commit provider keys, OAuth tokens, cookies, `.env` files, session DBs, or
provider logs. If a secret is exposed, revoke it at the provider first; deleting
it from the latest commit is not sufficient.

Primary maintainer: Yohji Sakamoto (`yohji.sakamoto@gmail.com`).
