# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in echo, please report it privately
via GitHub's Private Vulnerability Reporting:

1. Go to https://github.com/AIWander/echo/security
2. Click "Report a vulnerability"
3. Fill in the form with as much detail as you can — repro steps, impact, your environment.

I'll respond within 5 business days.

## Supported Versions

echo is under active development. Only the latest release receives security
fixes. Older releases are not supported.

## Scope

In scope:
- The echo binary/library and any bundled assets
- Default configuration files

Out of scope:
- Third-party MCP servers, CLIs, or tools not built by this project
- User-modified configurations
- Issues in upstream Rust crates (report those to the crate maintainers; they'll be picked up by Dependabot)

Thanks for helping keep CPC users safe.
