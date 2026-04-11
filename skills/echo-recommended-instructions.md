# echo MCP Server — Recommended CLAUDE.md Instructions

Copy the block below into your CLAUDE.md (or project-level instructions).

---

```markdown
## echo MCP Server (v1.1.1)

echo is a local-first MCP server wrapping Ollama. It is fully model-agnostic:
every tool requires the model as a parameter. There are no defaults.

### Rules

1. **Always specify the model parameter.** Every echo tool that calls Ollama
   requires `model`. No exceptions. No defaults. If you omit it, the call fails.

2. **Run `echo:health` first.** Before using any echo tool in a new session,
   call `health` to see which Ollama models are available. Do not assume any
   specific model is pulled.

3. **Never swap embedding models silently.** The semantic index records its
   embedding model and vector dimension as metadata. Querying with a different
   model than what built the index will be rejected. Switching models requires
   `semantic_reindex` (destructive rebuild). The same discipline applies to
   `store_pattern` / `compare` — always use the same model for both.

4. **Use `smart_fetch` over `web_fetch` for long pages.** smart_fetch has a
   local model summarize the HTML first, sending Claude a ~500-token summary
   instead of thousands of tokens of raw HTML. 90%+ token savings. Use `focus`
   to guide extraction ("main points", "prices", "technical specs").

5. **Use `heuristics` for behavioral signal detection.** It's instant, free
   (no model call), and detects corrections, decisions, discoveries, frustration,
   anti-patterns, pivots, and hedging via regex. Reserve `analyze` for deep
   second-opinion work where you need a local model's perspective.

6. **smart_fetch content may be stale.** For time-sensitive data (prices, scores,
   live stats), verify smart_fetch summaries against a real-time source. The
   tool includes a currency warning when it detects potentially time-sensitive
   content.

### Model selection guidance

- **Embeddings** (embed, store_pattern, compare, semantic_search, semantic_reindex):
  Pick one model, stick with it. Dimension and quality matter more than speed.
- **Summarization** (smart_fetch): Speed > depth. 7B-class models are fine.
- **Analysis** (analyze): Depth > speed. 13B+ for reasoning. 70B for real analysis.
- **Heuristics**: No model used. Instant regex matching.
```

---

**Notes for users:**
- This block is self-contained. It does not reference specific model names as requirements.
- Adjust the model selection guidance based on your GPU capability and pulled models.
- The `health` call will always tell you what's currently available.
