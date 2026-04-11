---
name: echo
description: >
  Canonical skill for the echo MCP server (v1.1.1). Local-first, model-agnostic
  AI toolkit wrapping Ollama: embeddings, semantic search, generation, summarization,
  pattern matching, and HTML fetch+summarize. Every tool requires the model as a
  per-call parameter. No defaults, no hardcoded model names, no assumptions.
version: 1.1.1
trigger_on:
  - echo
  - ollama
  - semantic search
  - embeddings
  - smart_fetch
  - local model
  - pattern matching
  - heuristics
  - vector search
  - similarity
  - local AI
  - embed text
  - summarize URL
---

# echo MCP Server — Skill Reference (v1.1.1)

## Overview

echo is a local-first MCP server that wraps Ollama. It provides:

- **Embeddings** — generate vector representations of text
- **Semantic search** — query a file-backed vector index by meaning
- **Generation** — deep analysis of text using local LLMs
- **Fetching** — pull web pages, strip HTML, summarize with a local model
- **Pattern matching** — store labeled exemplars, compare new text against them
- **Heuristics** — instant regex-based signal detection (no model needed)
- **Scoring** — evaluate Claude responses against a fitness function

echo runs entirely on the user's machine. Data stays local. Models stay local.
Indexes stay local. There is no cloud dependency, no API key, no usage billing.

### What echo is NOT

echo is not a memory system. It does not persist conversation history, manage
context windows, or automatically learn from sessions. It is a toolkit — you
call it when you need embeddings, similarity, summarization, or pattern detection.
Memory systems (if any) are built on top of echo's primitives, not inside it.

### Differentiation

| Feature | echo | mem0 | Letta/MemGPT | Zep | Anthropic Memory |
|---------|------|------|--------------|-----|-----------------|
| Local-first | Yes | Cloud default | Cloud default | Cloud | Cloud |
| Model-agnostic | Yes, per-call | Tied to provider | Tied to provider | Tied to provider | Claude only |
| Data ownership | User's filesystem | Vendor DB | Vendor DB | Vendor DB | Anthropic |
| smart_fetch pattern | Yes | No | No | No | No |
| Cost | $0 (your GPU) | Per-request | Per-request | Per-request | Per-token |

---

## The Model-Agnostic Philosophy

This is the core design principle of echo v1.1.1 and the thing that distinguishes
it from every other "local AI" project.

### The rule

**Every echo tool that calls Ollama requires the model as a parameter. There are
no defaults. There are no fallbacks. If you don't pass a model, the call fails.**

This is intentional. Here's why:

1. **No silent assumptions.** A tool that defaults to `nomic-embed-text` works
   great until someone pulls `mxbai-embed-large` and wonders why their index
   is broken. echo refuses to guess.

2. **Explicit is debuggable.** When every call names its model, you can trace
   exactly what produced every embedding, every summary, every analysis. No
   "which model was running when this index was built?" mysteries.

3. **The user is the authority.** Different tasks want different models. Embeddings
   want small and fast. Analysis wants large and deep. Summarization wants
   balanced. The caller knows their task — echo doesn't.

4. **Mixing models is a data corruption vector.** An embedding index built with
   model A is garbage if you query it with model B. echo's semantic store records
   the embedding model and vector dimension as metadata. If you try to query with
   a different model or dimension, echo refuses with a clear error. This is a
   feature, not a limitation.

### What this means in practice

Every time you call an echo tool, you must know:
- What Ollama models are available (run `health` first)
- Which model is appropriate for the task (see Picking Models below)
- Which model was used to build any existing index (for search/compare)

If you don't know what's available, **always start with `health`**. It lists
every pulled model with metadata. No assumptions required.

### The rebuild discipline

Switching embedding models means rebuilding your semantic index from scratch.
There is no migration path. echo enforces this:

- The semantic store records the model name and vector dimension on creation
- Queries with a mismatched model are rejected with a clear error message
- `semantic_reindex` blows away the existing index and rebuilds from all indexed files
- Existing indexes from echo versions before v1.1.1 are unreadable — rebuild fresh

This is the cost of model agnosticism: you own the choice, and you own the
consequences of changing it. The benefit is that you're never locked in.

---

## Picking Models for Each Tool

echo doesn't tell you which model to use. But here's a framework for choosing.

### Embedding tools: `embed`, `store_pattern`, `compare`, `semantic_search`, `semantic_reindex`

What matters for embeddings:
- **Dimension** — smaller dimensions = less storage, faster search, slightly less nuance
- **Multilingual support** — if your content isn't English-only, this matters
- **Retrieval quality** — measured by benchmarks like MTEB, but real-world testing is king

General guidance:
- Start with whatever embedding model you've already pulled
- If you're building a new index, pick one model and stick with it
- Test with your actual content before committing — benchmark scores don't always predict your domain
- Dimension is recorded in the store metadata; you cannot mix dimensions

### Generation tools: `smart_fetch`, `analyze`

These tools use Ollama's generation endpoint. Different tasks want different tradeoffs:

**For `smart_fetch` (summarization):**
- Speed matters more than depth — you're summarizing web pages, not writing dissertations
- 7B-class models are typically sufficient for extracting main points, prices, contact info
- Larger models add latency without proportional quality gain for shallow summarization
- Set `timeout_secs` appropriately — 60s default is generous for 7B, tight for 70B

**For `analyze` (deep retrospection):**
- Depth matters more than speed — you're asking for insight, not extraction
- 13B+ class is the floor for non-trivial reasoning about text
- 70B class for actual analytical work (corrections, decisions, emotional patterns)
- The `focus` parameter narrows the analysis: `corrections`, `decisions`, `emotions`, `authenticity`
- Expect 30-120 seconds depending on model size and text length

### Tools that ignore the model parameter

**`heuristics`** — pure regex pattern matching. No model involved. Instant.
Detects: corrections, decisions, discoveries, success, frustration, anti-patterns,
pivots, hedging. The `model` parameter is not accepted.

**`score_response`** — evaluates against a fitness function. No model needed.

**`health`** — checks Ollama reachability. No model needed.

**`server_health`** — checks MCP server processes. No model needed.

**`error_fallbacks`** — queries learned error patterns. No model needed.

**`plan`** — task decomposition. No model needed.

---

## Tool Reference

### Embedding Tools

#### `embed`

Generate a vector embedding for a piece of text.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `text` | Yes | Text to embed |
| `model` | Yes | Ollama embedding model name |

Returns the embedding vector. Use this when you need raw vectors for custom
similarity logic, clustering, or piping to external systems.

#### `store_pattern`

Store a labeled exemplar embedding for later comparison.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `label` | Yes | Human-readable label for this pattern |
| `text` | Yes | Example text representing this pattern |
| `model` | Yes | Ollama embedding model name |

Builds up a library of "what does X look like?" patterns. Used with `compare`
to classify new text against known exemplars.

#### `compare`

Compare text against all stored patterns, returns similarity scores.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `text` | Yes | Text to compare against stored patterns |
| `model` | Yes | Ollama embedding model (**must match** model used in `store_pattern`) |

Returns ranked similarity scores against every stored pattern. Use the same
embedding model you used when storing patterns — mismatched models produce
meaningless similarity scores.

### Semantic Search Tools

#### `semantic_search`

Query the semantic index by meaning similarity.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `query` | Yes | Natural language query |
| `model` | Yes | Embedding model (**must match** the model used at reindex time) |
| `limit` | No | Max results (default: 10) |

Searches the chunked, embedded index of your files. The model must match
what was used during `semantic_reindex` — echo checks store metadata and
rejects mismatches.

#### `semantic_reindex`

Rebuild the semantic search index from scratch.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `model` | Yes | Embedding model to use. Must be pulled in Ollama first. |

**Destructive operation.** Blows away the existing index entirely. Chunks all
indexed files and generates fresh embeddings. Vector dimension is auto-discovered
from the model's first output and recorded as metadata.

Run this:
- After pulling a new embedding model you want to switch to
- After significant content changes in your indexed files
- After upgrading from a pre-v1.1.1 echo (old indexes are unreadable)

### Generation Tools

#### `analyze`

Deep retrospection analysis of text using a local generation model.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `text` | Yes | Transcript or text to analyze |
| `model` | Yes | Ollama generation model name |
| `focus` | No | Focus area: `corrections`, `decisions`, `emotions`, `authenticity` |

Thorough but slow. Use for substantive analysis where you want a local model's
perspective. Not a substitute for Claude's reasoning — it's a second opinion
from a different model running locally.

#### `smart_fetch`

Fetch a URL, strip HTML, summarize with a local model.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `url` | Yes | URL to fetch |
| `model` | Yes | Ollama generation model for summarization |
| `focus` | No | What to extract: `main points`, `prices`, `contact info`, etc. |
| `max_tokens` | No | Max summary length (default: 500) |
| `timeout_secs` | No | LLM timeout in seconds (default: 60) |
| `skip_summary` | No | Just fetch and clean HTML, skip LLM (default: false) |
| `include_raw` | No | Include first 1000 chars of raw text (default: false) |

**The token saver.** Instead of pulling full HTML through Claude (expensive),
have echo's local model summarize first, send the summary to Claude. 90%+ token
reduction on long pages. Includes an instant currency check that warns if
content may need web_search verification for time-sensitive data.

Use `skip_summary: true` when you just want clean text without LLM processing.
Use `include_raw: true` when you need to verify the summary against source text.

### Pattern Matching Tools

#### `heuristics`

Instant regex-based pattern detection. No model, no API, no latency.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `text` | Yes | Text to analyze |

Detects behavioral signals:
- **Corrections** — "actually", "no wait", "that's wrong"
- **Decisions** — "let's go with", "I'll use", "decided to"
- **Discoveries** — "found that", "turns out", "realized"
- **Success** — "works", "fixed", "done"
- **Frustration** — "still broken", "why does", "keeps failing"
- **Anti-patterns** — known bad practices
- **Pivots** — "instead", "switching to", "new approach"
- **Hedging** — "maybe", "might", "not sure if"

Free, fast, and useful for behavioral signal extraction in automated pipelines.
No model overhead means you can run this on every turn without cost.

### System Tools

#### `health`

Check Ollama reachability and list all pulled models with metadata.

| Parameter | Required | Description |
|-----------|----------|-------------|
| (none) | | |

**Always call this first** in a new session or when you're unsure what models
are available. Returns:
- Ollama connection status
- List of every pulled model with name, size, and capabilities
- No assumptions about which models should be present

#### `score_response`

Score a Claude response against a fitness function.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `response` | Yes | The response text to score |
| `context` | No | The user query for context |

Returns a breakdown of deductions and bonuses. Useful for quality monitoring
and response tuning.

#### `server_health`

Check which MCP servers are alive.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `servers` | No | Specific servers to check (default: all) |

Returns process status for MCP servers. Not echo-specific — can check any server.

#### `mcp_rebuild`

Rebuild an MCP server binary with backup.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `target` | Yes | Server name to rebuild |

Backs up the existing exe, kills the process, runs `cargo build`. Use for
deploying new versions of any MCP server, not just echo.

#### `error_fallbacks`

Query learned error-to-fallback mappings.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `error_pattern` | No | Error text to match (omit to list all) |

Returns known error patterns and their recommended fallback actions.

#### `plan`

Analyze a task and return its ingredients.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `task` | Yes | What needs to be done |
| `context` | No | Additional context |

Returns what tools are needed, dependency order, and whether breadcrumbing is
warranted. Does not prescribe execution order — the caller decides.

---

## Common Patterns

### Pattern 1: First-session bootstrap

Always start a session that will use echo with a health check:

```
1. echo:health                          → see what's pulled
2. echo:semantic_search(query, model)   → test that the index works with your model
```

If `semantic_search` fails with a model mismatch error, you need to reindex.

### Pattern 2: smart_fetch as token saver

Instead of:
```
web_fetch(url) → Claude processes full HTML (thousands of tokens)
```

Do:
```
echo:smart_fetch(url, model, focus="main points") → Claude gets a 500-token summary
```

90%+ token reduction. The local model does the heavy lifting of HTML cleanup and
extraction. Claude gets a clean summary to reason about.

Use `focus` to guide extraction: `"prices"`, `"contact info"`, `"main points"`,
`"technical specs"`, etc.

For time-sensitive content (stock prices, weather, sports scores), smart_fetch
includes a currency warning. If you see it, verify with a real-time source.

### Pattern 3: The compare + store_pattern loop

Build a local similarity classifier:

```
1. echo:store_pattern(label="bug_report", text="...", model=M)
2. echo:store_pattern(label="feature_request", text="...", model=M)
3. echo:store_pattern(label="question", text="...", model=M)
4. echo:compare(text="new incoming text", model=M) → ranked similarity scores
```

Use this to classify text against known exemplars without cloud vector DBs.
All patterns are stored locally. Use the **same model** for store and compare.

### Pattern 4: heuristics for behavioral pipeline

Run `heuristics` on every significant text block to detect behavioral signals:

```
echo:heuristics(text="actually no, let's switch to the other approach")
→ detects: correction, pivot
```

Zero cost (no model call). Useful in extraction pipelines where you want to
flag corrections, decisions, and discoveries automatically before deciding
whether to persist them.

### Pattern 5: analyze for deep second opinion

When you want a local model's take on a transcript or text block:

```
echo:analyze(text="...", model=M, focus="corrections")
```

This is not a replacement for Claude's reasoning. It's a second opinion from
a different model. Useful for:
- Checking if a correction was actually meaningful
- Getting a different perspective on a decision
- Detecting emotional signals Claude might frame differently

### Pattern 6: Semantic search for knowledge retrieval

```
echo:semantic_search(query="how does breadcrumb tracking work", model=M, limit=5)
```

Searches chunked indexed files by meaning similarity. Returns the most relevant
chunks. Model must match what was used during `semantic_reindex`.

### Pattern 7: Embedding pipeline for custom applications

```
1. echo:embed(text="document A", model=M) → vector A
2. echo:embed(text="document B", model=M) → vector B
3. Compute similarity, cluster, or pipe to external tools
```

Raw embeddings for when you need vectors outside of echo's built-in search.

---

## Anti-Patterns

### DO NOT: Omit the model parameter

Every echo tool that talks to Ollama requires `model`. There are no defaults.
If you see a tool call without `model`, it's wrong.

### DO NOT: Mix embedding models

If you built your index with model A, you must query with model A. If you stored
patterns with model A, you must compare with model A. echo enforces this for
the semantic store (metadata check) but pattern store mismatches produce garbage
silently. Be disciplined.

### DO NOT: Use smart_fetch for time-sensitive data without verification

smart_fetch summarizes cached/fetched content through a local model. The content
may be stale, the model may hallucinate details. For prices, scores, or anything
time-sensitive, verify the summary against a real-time source.

### DO NOT: Use analyze for quick extraction

`analyze` calls a local generation model — it's slow (30-120s). If you just need
to detect patterns (correction, decision, discovery), use `heuristics` instead.
It's instant and free.

### DO NOT: Assume a model is pulled

Run `health` first. If your model isn't listed, it's not available. Pull it in
Ollama before calling echo tools.

### DO NOT: Reindex casually

`semantic_reindex` is destructive. It blows away the entire index and rebuilds.
On large file collections, this takes minutes. Don't trigger it unless you're
intentionally switching models or recovering from corruption.

### DO NOT: Use echo for conversation memory

echo provides primitives (embeddings, search, patterns). It does not manage
conversation history, session state, or automatic learning. If you need memory,
build it on top of echo's tools — don't expect echo to remember anything
between calls.

### DO NOT: Send huge text blocks to analyze

Local models have context limits. A 70B model with 4K context will truncate
your 50-page transcript silently. Keep text blocks reasonable for the model
you're using. When in doubt, chunk first.

---

## Troubleshooting

### "Connection refused" or "Ollama not reachable"

Ollama isn't running. Start it:
```
ollama serve
```
Then retry `health`.

### "Model not found" or empty health response

The model isn't pulled. Pull it:
```
ollama pull <model-name>
```
Common embedding models: `nomic-embed-text`, `mxbai-embed-large`, `bge-large-en-v1.5`
Common generation models: `mistral:7b`, `llama3.2`, `qwen2.5:14b`, `deepseek-r1:70b`

### "Dimension mismatch" on semantic_search

Your query model produces vectors of a different dimension than the index.
This happens when you switch models without reindexing. Fix:
```
echo:semantic_reindex(model="<your-current-model>")
```

### "Unreadable index" after upgrading to v1.1.1

echo v1.1.1 changed the semantic store format. Old indexes from earlier versions
cannot be read. Rebuild:
```
echo:semantic_reindex(model="<your-model>")
```

### smart_fetch times out

The local model is too slow for the content length. Options:
- Increase `timeout_secs` (e.g., 120 or 180)
- Use a smaller/faster model
- Use `skip_summary: true` to just get clean text without LLM processing

### compare returns nonsensical scores

You're likely using a different model than what was used in `store_pattern`.
Pattern store doesn't enforce model consistency (unlike semantic store). Always
use the same model for store and compare.

### analyze returns shallow or truncated results

The model's context window may be too small for your text. Either:
- Use a model with a larger context window
- Chunk the text and analyze pieces separately
- Use `focus` to narrow what the model looks for

### heuristics misses a pattern

heuristics is regex-based. It catches common phrasings but not every possible
way to express a correction, decision, or discovery. For nuanced detection,
combine heuristics (fast/free) with analyze (slow/thorough) on flagged segments.

### "echo server not responding"

Check if the echo process is running:
```
echo:server_health(servers=["echo"])
```

If it's down, rebuild:
```
echo:mcp_rebuild(target="echo")
```
