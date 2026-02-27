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

The application currently has Content Security Policy (CSP) disabled (`"csp": null` in `src-tauri/tauri.conf.json`). This is a known limitation.

**Why this is acceptable:**

- Glide only loads local assets from the application bundle
- No remote content is loaded or executed
- The application does not fetch resources from external URLs
- All functionality is self-contained within the Tauri application

**Future improvements:**

- Implementing a strict CSP is planned for a future update
- This will further harden the application's security posture

## Security Best Practices

When using Glide:

- Keep your Windows system and all software up to date
- Only download Glide from the official [GitHub Releases](https://github.com/wkdev/glide/releases) page
- Verify installer signatures when available
- Report any suspicious behavior to the security team

## License

Glide is released under the MIT License. See the LICENSE file for details.
