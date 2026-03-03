# Security Policy

## Supported Versions

The following versions of Glide are currently supported with security updates:

| Version | Status       |
| ------- | ------------ |
| 0.1.x   | ✅ Supported |

## Reporting a Vulnerability

We take security seriously. If you discover a vulnerability in Glide, please report it responsibly using GitHub Security Advisories.

### How to Report

1. Go to the [Security Advisories](https://github.com/wkdev/glide/security/advisories) page on GitHub
2. Click "Report a vulnerability"
3. Provide a detailed description of the vulnerability, including:
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if applicable)

### Response Timeline

- **48 hours**: We will acknowledge receipt of your report
- **7 days**: Initial assessment and next steps will be communicated
- **30 days**: Target for patch release (timeline may vary based on severity)

Please do not disclose the vulnerability publicly until we have released a fix.

## Known Security Considerations

### Content Security Policy (CSP)

The application enforces a strict Content Security Policy:

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'
```

This policy:

- Blocks all external resource loading (scripts, styles, images, fonts)
- Allows only bundled application assets (`'self'`)
- Permits inline styles (`'unsafe-inline'`) required by the UI framework
- Does not allow `eval()` or WebAssembly execution

### Logging

Application logs are stored in `AppData/Local/com.wkdev.glide/logs/` with automatic file rotation (50KB per file). Logs contain operational information only — no sensitive user data is recorded.

### Crash Reporting

Crash reporting via Sentry is available when the application is built with a `SENTRY_DSN` environment variable. When no DSN is provided (default), `sentry::init` is called with an empty DSN string, which causes the Sentry SDK to operate in a no-op mode with no network connections or data transmission. Crash reports capture panic backtraces only — no window titles, process names, or user activity data is transmitted.

## Security Best Practices

When using Glide:

- Keep your Windows system and all software up to date
- Only download Glide from the official [GitHub Releases](https://github.com/wkdev/glide/releases) page
- Verify installer signatures when available
- Report any suspicious behavior to the security team

## License

Glide is released under the MIT License. See the LICENSE file for details.
