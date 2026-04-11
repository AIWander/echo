# Pattern Learning

How to use echo's `store_pattern` and `compare` tools to build a local similarity classifier.

## Concept

Store labeled examples of text categories, then classify new text by comparing it against all stored patterns. No cloud vector DB, no API keys — everything runs locally.

## Step 1: Store exemplar patterns

```
echo:store_pattern(
  label="bug_report",
  text="The application crashes when I click the save button. I get a null pointer exception in the console.",
  model="nomic-embed-text"
)

echo:store_pattern(
  label="feature_request",
  text="It would be great if the app could export data to CSV format for spreadsheet analysis.",
  model="nomic-embed-text"
)

echo:store_pattern(
  label="question",
  text="How do I configure the database connection string? The docs don't mention environment variables.",
  model="nomic-embed-text"
)
```

Each call embeds the text and stores it with the label. More exemplars per category = better classification.

## Step 2: Classify new text

```
echo:compare(
  text="My program freezes every time I try to open a large file",
  model="nomic-embed-text"
)
```

Returns ranked similarity scores against every stored pattern:

```
bug_report:    0.87
question:      0.42
feature_request: 0.31
```

The highest score is the best match.

## Step 3: Build richer categories

Add multiple exemplars per label to cover different phrasings:

```
echo:store_pattern(label="bug_report", text="Error 500 on the login page since yesterday's deploy", model="nomic-embed-text")
echo:store_pattern(label="bug_report", text="The search results are returning duplicates that shouldn't exist", model="nomic-embed-text")
```

More variety in exemplars = more robust classification.

## Critical Rule: Model Consistency

**Always use the same embedding model for `store_pattern` and `compare`.**

Unlike the semantic store, the pattern store does not enforce model consistency. If you store with `nomic-embed-text` and compare with `mxbai-embed-large`, you'll get meaningless scores without any error. Be disciplined.

## Use Cases

- **Ticket triage** — classify incoming tickets as bugs, features, questions
- **Content categorization** — sort documents by topic
- **Behavioral detection** — store examples of corrections, decisions, discoveries and classify conversation turns
- **Anomaly detection** — if no stored pattern scores above a threshold, the text is novel
