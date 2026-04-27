# Contributing to echo

Thanks for your interest in contributing to echo.

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Install the Rust toolchain (stable)
4. Install Ollama and pull at least one embedding model
5. Build: `cargo build --release`
6. Run tests: `cargo test`

## Development Setup

```bash
# Clone
git clone https://github.com/AIWander/echo.git
cd echo

# Build
cargo build --release

# Run locally
cargo run -- --stdio
```

### Prerequisites

- Rust stable toolchain
- Ollama installed and running (`ollama serve`)
- At least one embedding model pulled (e.g., `ollama pull nomic-embed-text`)

## What to Contribute

- Bug fixes with test coverage
- Performance improvements (echo should be fast — it's a local tool)
- New heuristic patterns for behavioral signal detection
- Documentation improvements
- Platform support (Linux, macOS)

## Guidelines

### Code Style

- Follow existing Rust conventions in the codebase
- Run `cargo clippy` before submitting
- Run `cargo fmt` before submitting
- All public functions need doc comments

### Model Agnosticism

This is echo's core principle. Any contribution must maintain it:

- **Never** hardcode a model name as a default
- **Never** assume a specific model is available
- **Always** require the `model` parameter on tools that call Ollama
- **Always** validate model/dimension consistency on indexed data

If your change introduces a new tool that calls Ollama, it must accept `model` as a required parameter.

### Pull Requests

1. Create a feature branch from `main`
2. Keep changes focused — one feature or fix per PR
3. Include tests for new functionality
4. Update the skill file (`skills/echo.md`) if you add or change tools
5. Update `CHANGELOG.md` with your changes under `[Unreleased]`

### Commit Messages

Use clear, descriptive commit messages:

```
Add timeout parameter to analyze tool

The analyze tool could hang indefinitely on large inputs with slow
models. Added a configurable timeout_secs parameter (default: 120).
```

### Testing

- Unit tests for new functions
- Integration tests that verify Ollama interaction (these require Ollama running)
- Mark Ollama-dependent tests with `#[ignore]` so CI can skip them without Ollama

## Reporting Issues

Open an issue on GitHub with:

- echo version (`v1.1.1` etc.)
- Your platform (Windows x64/ARM64, etc.)
- Ollama version and models involved
- Steps to reproduce
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.

## Contact

- GitHub: [AIWander](https://github.com/AIWander/)
- Email: protipsinc@gmail.com
