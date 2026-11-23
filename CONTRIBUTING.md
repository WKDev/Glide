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
- **Node.js 24** — [Install Node.js](https://nodejs.org/)
- **pnpm 9** — Install via `npm install -g pnpm@9`
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

The GitHub distribution uses [Tauri update signing](https://v2.tauri.app/plugin/updater/) to authenticate updates. The Microsoft Store distribution uses Store-managed signing and updates.

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

The updater public key is stored in `src-tauri/tauri.conf.json`. Existing installations verify updates against the key embedded in their build, so plan any key rotation before changing it.

```json
"plugins": {
  "updater": {
    "pubkey": "<paste full content of glide.key.pub here>",
    ...
  }
}
```

### GitHub release

1. Set the intended version in `package.json` and update the changelog.
2. Push the matching `v<version>` tag to run the release workflow.
3. Check the published NSIS and MSI installers and the `latest.json` update manifest. From an older GitHub build, open Settings → About to check for the update. Verify that updating preserves existing settings.

Tauri's update signature authenticates the download within Glide. Windows publisher trust for directly distributed installers uses Windows code signing.

### Microsoft Store submission

1. Reserve the product in Partner Center. In GitHub repository variables, set `MSIX_IDENTITY_NAME`, `MSIX_PUBLISHER`, and `MSIX_PUBLISHER_DISPLAY_NAME` to the corresponding values under the product identity.
2. Run **Microsoft Store MSIX** in GitHub Actions for the intended ref. Download the `glide-store-msix` artifact and upload its MSIX in Partner Center.
3. Validate the Store package, then test a fresh installation and an upgrade with the same package identity. Check the tray, startup toggle, settings persistence, window controls, and Store-managed updates on Windows.

Microsoft signs and hosts the package after Store certification. If you sideload an MSIX for local testing, Windows must trust the certificate used to sign that package. [Microsoft Store signing overview](https://learn.microsoft.com/windows/apps/publish/faq/get-started-with-the-microsoft-store)

### Windows smoke test

- Try default move and resize, including release of the mouse button or modifier while the pointer is stationary. Holding just the modifier should leave windows alone.
- Try edge and corner snaps on two monitors with different scales, including a maximized window and a background target. A shortcut during a drag should cancel the snap.
- Verify whitelist and blacklist behavior for moving, resizing, and scrolling. Check that an excluded app still receives its normal wheel input.
- Change settings while a previous write is in flight, then restart and verify the last value. Pause from the tray, save a setting, and verify the pause survives.
- Exercise Windows 10 and 11, ordinary and elevated target windows, and the relevant installer or Store update path.
