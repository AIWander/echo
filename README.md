# echo MCP Server

**Model-agnostic local AI for Claude Desktop.**

echo wraps [Ollama](https://ollama.com) as an MCP server — giving Claude semantic search, embeddings, retrospection analysis, error pattern learning, and web summarization using **any local model you choose**.

## The Model-Agnostic Philosophy

This is what makes echo different from every other "local AI" integration.

**Every echo tool that calls Ollama requires the model as a parameter. There are no defaults. There are no fallbacks. If you don't pass a model, the call fails.**

Why:

- **No silent assumptions.** A tool that defaults to `nomic-embed-text` works great until someone pulls `mxbai-embed-large` and wonders why their index is broken. echo refuses to guess.
- **Explicit is debuggable.** Every call names its model. You can trace exactly what produced every embedding, every summary, every analysis.
- **The user is the authority.** Embeddings want small and fast. Analysis wants large and deep. The caller knows their task — echo doesn't.
- **Mixing models is a data corruption vector.** An embedding index built with model A is garbage if you query it with model B. echo enforces this with metadata checks and clear errors.

You pick the Ollama model. echo runs it. Your data stays on your machine.

## Capabilities

| Category | Tools | Model Required |
|----------|-------|----------------|
| **Embeddings** | `embed`, `store_pattern`, `compare` | Yes |
| **Semantic Search** | `semantic_search`, `semantic_reindex` | Yes |
| **Generation** | `analyze` (deep retrospection), `smart_fetch` (URL summarization) | Yes |
| **Pattern Detection** | `heuristics` (regex-based behavioral signals) | No — instant |
| **Scoring** | `score_response` (fitness function evaluation) | No |
| **Error Learning** | `error_fallbacks` (learned error-to-fix mappings) | No |
| **System** | `health`, `server_health`, `mcp_rebuild`, `plan` | No |

### Highlights

- **Semantic search** — query your files by meaning, not keywords. Index is chunked and embedded locally.
- **Pattern storage & comparison** — build a local similarity classifier. Store labeled exemplars, compare new text against them.
- **smart_fetch** — fetch a URL, strip HTML, summarize with a local model. 90%+ token savings vs sending raw HTML to Claude.
- **Heuristics** — instant regex detection of corrections, decisions, discoveries, frustration, pivots, hedging. Zero cost.
- **Retrospection analysis** — deep second-opinion from a local model on any text block.
- **Error fallback learning** — query known error patterns and their recommended fixes.

## Requirements

- **Ollama** installed and running (`ollama serve`)
- At least one model pulled (e.g., `ollama pull nomic-embed-text`)
- **Claude Desktop** with MCP support
- Windows x64 or ARM64

## Installation

1. Download the echo binary for your platform from [Releases](https://github.com/josephwander-arch/echo/releases):
   - `echo_x64_windows.exe` for x64
   - `echo_arm64_windows.exe` for ARM64

2. Place the binary somewhere permanent (e.g., `C:\CPC\servers\echo.exe`).

3. Install and start Ollama:
   ```
   # Download from https://ollama.com
   ollama serve
   ```

4. Pull at least one model:
   ```
   # For embeddings
   ollama pull nomic-embed-text

   # For generation/summarization
   ollama pull mistral:7b
   ```

5. Add echo to your Claude Desktop config (`claude_desktop_config.json`):
   ```json
   {
     "mcpServers": {
       "echo": {
         "command": "C:\\CPC\\servers\\echo.exe",
         "args": []
       }
     }
   }
   ```

   See `claude_desktop_config.example.json` for full ARM64 + x64 examples.

6. Restart Claude Desktop.

7. Verify: ask Claude to run `echo:health` — it should list your pulled models.

## Picking Your Models

echo doesn't tell you which model to use. Here's a framework:

### For embeddings (`embed`, `store_pattern`, `compare`, `semantic_search`, `semantic_reindex`)
- Pick one model and **stick with it** — mixing models corrupts your index
- Common choices: `nomic-embed-text`, `mxbai-embed-large`, `bge-large-en-v1.5`
- Test with your actual content before committing

### For summarization (`smart_fetch`)
- Speed matters more than depth — 7B-class models are sufficient
- Common choices: `mistral:7b`, `llama3.2`, `qwen2.5:7b`
- Adjust `timeout_secs` for larger models

### For analysis (`analyze`)
- Depth matters more than speed — 13B+ for reasoning, 70B for serious analysis
- Common choices: `qwen2.5:14b`, `deepseek-r1:70b`, `llama3.1:70b`
- Use the `focus` parameter to narrow analysis scope

### No model needed
- `heuristics` — instant regex pattern detection
- `score_response`, `error_fallbacks`, `health`, `server_health`, `plan`

## Quick Start

```
# 1. Check what models are available
echo:health

# 2. Search your indexed files by meaning
echo:semantic_search(query="how does authentication work", model="nomic-embed-text", limit=5)

# 3. Summarize a web page locally (saves 90%+ tokens)
echo:smart_fetch(url="https://example.com/docs", model="mistral:7b", focus="main points")

# 4. Detect behavioral signals instantly
echo:heuristics(text="actually no, let's switch to the other approach")
# → detects: correction, pivot

# 5. Store and compare patterns
echo:store_pattern(label="bug_report", text="the app crashes when...", model="nomic-embed-text")
echo:compare(text="my program freezes on startup", model="nomic-embed-text")
```

## Diagnostics

Run `doctor.ps1` to check your installation:

```powershell
.\doctor.ps1
```

It verifies:
- echo binary is present and executable
- Ollama is reachable at `localhost:11434`
- Lists all available Ollama models


## Anti-Patterns

- **Don't omit the model parameter.** Every Ollama-backed tool requires it. No defaults.
- **Don't mix embedding models.** Index built with model A must be queried with model A.
- **Don't trust smart_fetch for time-sensitive data.** Verify prices, scores, and live stats elsewhere.
- **Don't use `analyze` for quick detection.** Use `heuristics` — it's instant and free.
- **Don't assume a model is pulled.** Run `health` first.
- **Don't reindex casually.** `semantic_reindex` destroys and rebuilds the entire index.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "Connection refused" | Start Ollama: `ollama serve` |
| "Model not found" | Pull it: `ollama pull <model-name>` |
| "Dimension mismatch" on search | Reindex: `echo:semantic_reindex(model="<your-model>")` |
| "Unreadable index" after upgrade | Rebuild: `echo:semantic_reindex(model="<your-model>")` |
| smart_fetch timeout | Increase `timeout_secs`, use smaller model, or set `skip_summary: true` |
| compare returns nonsense | Use same model for `store_pattern` and `compare` |

---

## Compatible With

Works with any MCP client. Common install channels:

- **Claude Desktop** (the main chat app) — add to `claude_desktop_config.json`. See `claude_desktop_config.example.json` in this repo.
- **Claude Code** — add to `~/.claude/mcp.json`, or point your `CLAUDE.md` at `skills/echo.md` to load it as a skill instead.
- **OpenAI Codex CLI** — register via Codex's MCP config, or load the skill directly.
- **Gemini CLI** — register via Gemini's MCP config, or load the skill directly.

**Two install layouts:**

1. **Local folder** — clone or download this repo, then point your client at the local directory or the extracted `.exe` binary.
2. **Installed binary** — grab the `.exe` from the [Releases](https://github.com/josephwander-arch/echo/releases) page, place it wherever you keep your MCP binaries, then register its path in your client's config.

**Also ships as a skill** — if your client supports Anthropic skill files, load `skills/echo.md` directly. Skill-only mode gives you the behavioral guidance without running the server; useful for planning, review, or read-only workflows.

### First-run tip: enable "always-loaded tools"

For the smoothest experience, enable **tools always loaded** in your Claude client settings (Claude Desktop: Settings → Tools, or equivalent in Claude Code / Codex / Gemini). This ensures Claude recognizes the tool surface on first use without needing to re-discover it every session. Most users hit friction on day one because this is off by default.

## License

Apache License 2.0 — see [LICENSE](LICENSE).

## Donations

If echo saves you time or tokens: **$NeverRemember** (Cash App)

## Contact

- GitHub: [josephwander-arch](https://github.com/josephwander-arch/)
- Email: protipsinc@gmail.com

---

*echo is part of the CPC (Cognitive Performance Computing) platform.*
