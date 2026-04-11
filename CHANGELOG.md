# Changelog

All notable changes to echo will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [1.1.1] - 2026-04-11

Initial public release.

### Features

- **Model-agnostic design** — every Ollama-backed tool requires the model as a parameter. No defaults, no assumptions.
- **embed** — generate vector embeddings for text using any Ollama embedding model
- **store_pattern** / **compare** — build a local similarity classifier with labeled exemplars
- **semantic_search** — query a file-backed vector index by meaning similarity
- **semantic_reindex** — destructive rebuild of the semantic index with a chosen model
- **analyze** — deep retrospection analysis using a local generation model
- **smart_fetch** — fetch URL, strip HTML, summarize with a local model (90%+ token savings)
- **heuristics** — instant regex-based behavioral signal detection (corrections, decisions, discoveries, frustration, pivots, hedging)
- **score_response** — evaluate responses against a fitness function
- **error_fallbacks** — query learned error-to-fallback mappings
- **health** — check Ollama reachability and list all pulled models
- **server_health** — check MCP server process status
- **mcp_rebuild** — rebuild an MCP server binary with backup
- **plan** — task decomposition and ingredient analysis

### Architecture

- Semantic store records embedding model and vector dimension as metadata
- Dimension mismatch protection on semantic_search queries
- Currency detection warnings on smart_fetch for time-sensitive content
- ARM64 and x64 Windows binaries

[1.1.1]: https://github.com/josephwander-arch/echo/releases/tag/v1.1.1
