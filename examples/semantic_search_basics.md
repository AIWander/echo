# Semantic Search Basics

How to use echo's semantic search to query your files by meaning.

## Prerequisites

- Ollama running with an embedding model pulled
- Files indexed via `semantic_reindex`

## Step 1: Check available models

```
echo:health
```

Note which embedding model you have (e.g., `nomic-embed-text`).

## Step 2: Build the index

```
echo:semantic_reindex(model="nomic-embed-text")
```

This chunks all indexed files and generates embeddings. It's destructive — any existing index is wiped and rebuilt.

The store records the model name and vector dimension as metadata. You must use the same model for queries.

## Step 3: Search by meaning

```
echo:semantic_search(query="how does authentication work", model="nomic-embed-text", limit=5)
```

Returns the top 5 most semantically similar chunks from your indexed files.

## Step 4: Refine

- Adjust `limit` to get more or fewer results
- Make queries natural language — the embedding model understands meaning, not keywords
- If results seem off, check that your query model matches the index model

## Switching Models

If you want to try a different embedding model:

```
ollama pull mxbai-embed-large
echo:semantic_reindex(model="mxbai-embed-large")
```

You must rebuild the entire index. There is no migration path between models — different models produce different vector spaces.

## Common Mistakes

- **Using a different model for search than for indexing** — echo will reject the query with a dimension mismatch error
- **Searching before indexing** — run `semantic_reindex` first
- **Keyword-style queries** — "auth error 401" works better as "why is authentication failing with unauthorized errors"
