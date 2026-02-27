# Contributing to Glide

Thank you for your interest in contributing to Glide! This document provides guidelines and instructions for contributing to the project.

## How to Contribute

We follow a standard fork → branch → pull request workflow:

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/glide.git
   cd glide
   ```
3. **Create a feature branch** from `main`:
   ```bash
   git checkout -b feat/your-feature-name
   ```
4. **Make your changes** and commit using Conventional Commits (see below)
5. **Push to your fork**:
   ```bash
   git push origin feat/your-feature-name
   ```
6. **Open a Pull Request** on the main repository with a clear description of your changes

## Local Development Setup

### Prerequisites

Before you begin, ensure you have the following installed:

- **Rust toolchain** (stable) — [Install Rust](https://www.rust-lang.org/tools/install)
- **Node.js 18+** — [Install Node.js](https://nodejs.org/)
- **pnpm** — Install via `npm install -g pnpm`
- **Windows OS** — Glide is a Windows-only application

### Installation

1. Install dependencies:

   ```bash
   pnpm install
   ```

2. Start the development server:
   ```bash
   pnpm tauri dev
   ```

This will launch the Tauri development environment with hot-reload enabled.

## Code Style

### Frontend (TypeScript/Svelte)

We use ESLint and Prettier for code quality and formatting:

- **Lint your code**:

  ```bash
  pnpm lint
  ```

- **Check formatting**:

  ```bash
  pnpm format:check
  ```

- **Auto-format code**:
  ```bash
  pnpm format
  ```

### Rust Backend

We follow Rust conventions with `cargo fmt` and `clippy`:

- **Format Rust code**:

  ```bash
  cargo fmt
  ```

- **Run clippy linter**:
  ```bash
  cargo clippy
  ```

## Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for clear and semantic commit messages.

### Commit Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation changes
- **ci**: CI/CD configuration changes
- **chore**: Build process, dependencies, or other non-code changes
- **refactor**: Code refactoring without feature changes
- **test**: Adding or updating tests
- **perf**: Performance improvements

### Examples

```bash
git commit -m "feat(window): add modifier key detection for window move"
git commit -m "fix(tray): resolve icon rendering on startup"
git commit -m "docs: update README with setup instructions"
git commit -m "ci: add GitHub Actions workflow for tests"
```

## Pull Request Process

1. **Ensure your code follows the style guidelines** (run linters and formatters)
2. **Write clear commit messages** using Conventional Commits
3. **Provide a descriptive PR title and description**:
   - What problem does this solve?
   - How does it solve it?
   - Any breaking changes?
4. **Link related issues** (e.g., "Closes #123")
5. **Be responsive to review feedback** — we may request changes

## License

By contributing to Glide, you agree that your contributions will be licensed under the MIT License (see [LICENSE](LICENSE) file).

## Questions?

If you have questions or need help, feel free to:

- Open a GitHub issue for bugs or feature requests
- Check existing issues and discussions
- Review the [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

Happy contributing! 🚀

## Release & Signing

Glide uses [Tauri's update signing](https://tauri.app/plugin/updater/) to verify update authenticity. Maintainers need a signing key pair to publish releases.

### Generating a Signing Key

```bash
pnpm tauri signer generate -w ~/.tauri/glide.key
```

This creates two files:

- `~/.tauri/glide.key` — **private key** (keep secret, never commit)
- `~/.tauri/glide.key.pub` — **public key** (safe to share)

### Configuring GitHub Secrets

In your GitHub repository settings, add the following secrets:

| Secret                               | Value                                                            |
| ------------------------------------ | ---------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | Full content of `~/.tauri/glide.key`                             |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password you entered during key generation (leave empty if none) |

### Updating the Public Key

Replace `UPDATER_PUBKEY_PLACEHOLDER` in `src-tauri/tauri.conf.json` with the full content of `~/.tauri/glide.key.pub`:

```json
"plugins": {
  "updater": {
    "pubkey": "<paste full content of glide.key.pub here>",
    ...
  }
}
```

### Publishing a Release

1. Create a GitHub Release and **publish it** (do not leave as draft)
2. The release workflow will automatically build and attach the NSIS installer and `latest.json` update manifest
3. Users running Glide will detect the update on next launch

> ⚠️ **Important**: Draft releases are **not** detected by the auto-updater. You must publish the release for updates to be distributed.
