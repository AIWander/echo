# Error Fallback Learning

How to use echo's `error_fallbacks` tool to build an error-to-fix knowledge base.

## Concept

echo can store mappings from error patterns to their recommended fixes. Over time, this builds a local knowledge base of "when you see X, try Y" — learned from your actual environment and workflows.

## Querying Known Errors

Check if echo knows a fix for an error you're seeing:

```
echo:error_fallbacks(error_pattern="connection refused")
```

Returns any stored fallback actions that match the pattern. If no match exists, you get an empty result.

## Querying All Known Patterns

Omit the parameter to see every error-to-fix mapping:

```
echo:error_fallbacks()
```

Returns the full list of learned error patterns and their fallback actions.

## How It Works in Practice

### Scenario: Recurring build failure

1. You encounter `LINK : fatal error LNK1181: cannot open input file` repeatedly
2. The fix turns out to be killing a zombie process holding the file lock
3. The error-to-fix mapping gets stored in echo
4. Next time the error appears, `error_fallbacks` returns the fix immediately

### Scenario: Ollama connection issues

1. `Connection refused` errors happen when Ollama isn't running
2. The stored fallback: "Run `ollama serve` to start the Ollama daemon"
3. Any agent or workflow can query this instead of re-discovering the fix

## Integration with Automated Workflows

error_fallbacks is most powerful when integrated into automated error handling:

```
1. Tool call fails with an error
2. Query: echo:error_fallbacks(error_pattern="<the error text>")
3. If a match exists → apply the fallback automatically
4. If no match → escalate to the user, then store the resolution for next time
```

## Key Properties

- **No model required** — error_fallbacks is a lookup, not an inference
- **Instant** — pattern matching against stored entries, no LLM overhead
- **Grows over time** — the more errors you encounter and resolve, the smarter it gets
- **Local** — all error patterns stay on your machine
