# Contributing to Crux

Thank you for your interest in contributing to Crux! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- Rust stable toolchain (see `rust-toolchain.toml`)
- macOS 13+ (Ventura) for Metal rendering
- Xcode Command Line Tools

### Building

```bash
git clone https://github.com/user/crux.git
cd crux
cargo build
```

### Running Tests

```bash
cargo test --workspace
```

## How to Contribute

### Reporting Bugs

- Use the [Bug Report](https://github.com/user/crux/issues/new?template=bug_report.md) issue template
- Include your macOS version, Rust version, and steps to reproduce
- Attach terminal output or screenshots if applicable

### Suggesting Features

- Use the [Feature Request](https://github.com/user/crux/issues/new?template=feature_request.md) issue template
- Describe the use case and expected behavior

### Submitting Changes

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make your changes
4. Ensure all checks pass:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test --workspace
   ```
5. Commit following our [commit convention](#commit-convention)
6. Push to your fork and open a Pull Request

## Commit Convention

We use [Conventional Commits](https://www.conventionalcommits.org/) with crate-name scopes.

### Format

```
<type>(<scope>): <subject>
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`

**Scopes**: Use the crate name without `crux-` prefix — `protocol`, `terminal`, `terminal-view`, `app`, `ipc`, `clipboard`. Non-crate scopes: `deps`, `ci`, `workspace`, `release`.

### Rules

- Imperative mood, lowercase, no trailing period
- Subject line max 72 characters
- Body (optional): wrap at 72 chars, explain *why* not *what*
- Reference issues in footer: `Closes #123`

### Examples

```
feat(terminal): add sixel graphics rendering
fix(app): prevent panic on zero-size window resize
refactor(terminal-view): extract cursor rendering to separate method
build(deps): bump gpui to 0.2.3
docs: update installation instructions
```

See `CLAUDE.md` for the full convention reference.

## Code Style

- Run `cargo fmt` before committing
- Follow `clippy` recommendations (`cargo clippy -- -D warnings`)
- Keep functions focused and small
- Write tests for new functionality

## License

By contributing to Crux, you agree that your contributions will be dual-licensed under the MIT and Apache 2.0 licenses.
