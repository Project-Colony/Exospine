# Security Policy

Exospine reads mail from strangers: every message it downloads is untrusted
input, and it holds the credentials of every account you add. Security reports
are handled before any other work.

## Supported versions

Only the **latest release** receives security fixes. Colony, the Project Colony
app store, offers each new release as an update, so staying current is one
update away.

| Version        | Supported |
| -------------- | --------- |
| latest release | Yes       |
| anything older | No        |

## Reporting a vulnerability

Report vulnerabilities **privately** through
[GitHub Security Advisories](https://github.com/Project-Colony/Exospine/security/advisories/new)
("Report a vulnerability"). Do not open a public issue for an exploitable bug,
and do not attach real mail, passwords or tokens to a report.

What to expect:

- **Acknowledgement** within a few days.
- A fix, or a mitigation plan, before any public disclosure, coordinated with
  you. Releases are automated, so a patched release usually ships as soon as
  the fix lands.
- Credit in the release notes if you want it.

## Scope

Reports of particular interest:

- **Email content**: a message whose HTML runs script, submits a form, loads a
  remote resource before **Show images**, or otherwise escapes the sanitizer in
  the reader, reply, forward, print or pop-out views.
- **Credentials**: passwords or OAuth tokens that reach a file, a log or the
  webview, or that another local user or program can read.
- **OAuth sign-in**: anything that lets another process capture the redirect on
  `127.0.0.1:8844` or substitute its own tokens.
- **Release integrity**: a release asset whose `.sig`, `.meta` or `.meta.sig`
  does not verify against the organisation key in the README's
  [Code signing policy](README.md#code-signing-policy).

Out of scope: the security of your mail provider's own servers, and phishing that
the warnings do not catch (they are a help, not a guarantee).
