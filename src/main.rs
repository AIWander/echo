//! Retrospector MCP Server v1.3.0
//! Local AI-powered conversation analysis using Ollama — model-agnostic
//!
//! Tools:
//! - heuristics: Fast pattern matching (corrections, decisions, emotions)
//! - embed: Get embeddings via any Ollama embedding model (caller specifies)
//! - store_pattern: Save labeled patterns for comparison
//! - compare: Compare text against stored patterns
//! - analyze: Deep analysis via any Ollama generation model (caller specifies)
//! - smart_fetch: Fetch URL, strip HTML, summarize with any Ollama model (token saver!)
//! - health: Check Ollama connection and list available models
// NAV: TOC at line 1082 | 11 fn | 14 struct | 2026-04-11

mod semantic;
mod planner;

use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const OLLAMA_URL: &str = "http://localhost:11434";
const FALLBACK_MAP_PATH: &str = "C:\\My Drive\\Volumes\\system_architecture\\tool_fallback_map.json";
const ERROR_FALLBACKS_PATH: &str = "C:\\My Drive\\Volumes\\logs\\error_fallbacks.json";

// ============================================================================
// Ollama API Types
// ============================================================================

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    keep_alive: Option<String>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Deserialize)]
struct OllamaTagModel {
    name: String,
    size: Option<u64>,
    details: Option<OllamaTagModelDetails>,
}

#[derive(Deserialize)]
struct OllamaTagModelDetails {
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

// ============================================================================
// HTML Stripping & Text Processing
// ============================================================================

fn strip_html(html: &str) -> String {
    let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_head = Regex::new(r"(?is)<head[^>]*>.*?</head>").unwrap();
    let re_nav = Regex::new(r"(?is)<nav[^>]*>.*?</nav>").unwrap();
    let re_footer = Regex::new(r"(?is)<footer[^>]*>.*?</footer>").unwrap();
    
    let mut text = html.to_string();
    text = re_script.replace_all(&text, "").to_string();
    text = re_style.replace_all(&text, "").to_string();
    text = re_head.replace_all(&text, "").to_string();
    text = re_nav.replace_all(&text, "").to_string();
    text = re_footer.replace_all(&text, "").to_string();
    
    let re_blocks = Regex::new(r"(?i)</(p|div|h[1-6]|li|tr|br)[^>]*>").unwrap();
    text = re_blocks.replace_all(&text, "\n").to_string();
    
    let re_tags = Regex::new(r"<[^>]+>").unwrap();
    text = re_tags.replace_all(&text, "").to_string();
    
    text = text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    
    let re_whitespace = Regex::new(r"[ \t]+").unwrap();
    text = re_whitespace.replace_all(&text, " ").to_string();
    
    let re_newlines = Regex::new(r"\n\s*\n+").unwrap();
    text = re_newlines.replace_all(&text, "\n\n").to_string();
    
    text.trim().to_string()
}

fn truncate_to_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.len() <= max_chars {
        (text.to_string(), false)
    } else {
        let truncated = &text[..max_chars];
        if let Some(last_space) = truncated.rfind(' ') {
            (truncated[..last_space].to_string(), true)
        } else {
            (truncated.to_string(), true)
        }
    }
}

// ============================================================================
// Currency Detection (Instant - No Mistral)
// ============================================================================

#[derive(Debug, Clone, Serialize, Default)]
struct CurrencyCheck {
    needs_verification: bool,
    reasons: Vec<String>,
    time_sensitive_matches: Vec<String>,
}

fn check_currency_needs(text: &str) -> CurrencyCheck {
    let mut result = CurrencyCheck::default();
    let text_lower = text.to_lowercase();
    
    // Patterns that suggest time-sensitive content
    let patterns = [
        (r"(?i)(as of|updated|last updated)\s*(january|february|march|april|may|june|july|august|september|october|november|december|\d{1,2}[/-]\d{1,2})", "Contains date reference"),
        (r"(?i)(current(ly)?|now|today|this week|this month|latest)\s+(price|rate|status|ceo|president|leader)", "Current status reference"),
        (r"(?i)\$\d+[\d,]*(\.\d{2})?", "Contains prices"),
        (r"(?i)(stock|share|market)\s+(price|value|trading)", "Financial data"),
        (r"(?i)(election|vote|poll|approval rating)", "Political/polling data"),
        (r"(?i)(breaking|just (announced|released)|recently)", "Recent events language"),
        (r"(?i)(q[1-4]\s*20\d{2}|fy\s*20\d{2}|20\d{2}\s*(earnings|results|report))", "Financial period reference"),
    ];
    
    for (pattern, reason) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(m) = re.find(&text_lower) {
                result.needs_verification = true;
                if !result.reasons.contains(&reason.to_string()) {
                    result.reasons.push(reason.to_string());
                }
                // Capture a snippet
                let start = m.start().saturating_sub(20);
                let end = (m.end() + 30).min(text.len());
                let snippet = text[start..end].to_string();
                if result.time_sensitive_matches.len() < 3 {
                    result.time_sensitive_matches.push(snippet);
                }
            }
        }
    }
    
    result
}

// ============================================================================
// Heuristics Engine v2 â€” Enhanced with discourse markers, NLP lexicon patterns
// Sources: AFINN-165 valence, discourse marker taxonomy, conversation analysis
// 8 categories, ~200 patterns (up from 4 categories, ~30 patterns)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HeuristicResults {
    corrections: Vec<String>,    // Self-repair, mistakes caught â†’ behavioral_patterns
    decisions: Vec<String>,      // Choices made â†’ behavioral_patterns
    discoveries: Vec<String>,    // Things learned â†’ topic-dependent
    success: Vec<String>,        // Things that worked â†’ tool_selection, system_architecture
    frustration: Vec<String>,    // Pain points â†’ friction_reduction, anti_patterns
    anti_patterns: Vec<String>,  // Recognized mistakes â†’ anti_patterns
    pivots: Vec<String>,         // Topic changes â†’ extraction sweep trigger
    hedging: Vec<String>,        // Uncertainty signals â†’ held-back thoughts
    // Legacy aliases (kept for backward compat in JSON output)
    emotions: Vec<String>,       // Maps to frustration + success combined
    held_back: Vec<String>,      // Maps to hedging
    score: f32,
}

fn extract_context(text: &str, start: usize, end: usize, before: usize, after: usize) -> String {
    let mut ctx_start = start.saturating_sub(before);
    let mut ctx_end = (end + after).min(text.len());
    // Ensure we're on char boundaries
    while ctx_start > 0 && !text.is_char_boundary(ctx_start) { ctx_start -= 1; }
    while ctx_end < text.len() && !text.is_char_boundary(ctx_end) { ctx_end += 1; }
    text[ctx_start..ctx_end].to_string()
}

fn run_heuristics(text: &str) -> HeuristicResults {
    let mut results = HeuristicResults::default();
    
    // â”€â”€ CORRECTIONS: Self-repair & mistake acknowledgment â”€â”€
    // Discourse markers: rectifying reformulators, self-repair initiators
    // "actually" as topic-shift correction, "I mean" as reformulation
    if let Ok(re) = Regex::new(r"(?i)\b(actually[,.]?\s|no wait|I was wrong|let me correct|that's not right|I misspoke|not quite|scratch that|never mind|my bad|oops|my mistake|I meant|what I meant|let me rephrase|I stand corrected|correction:|hold on[,.]?\s|wait[,.]?\s(?:no|that)|rather[,.]?\s|instead[,.]?\s(?:of|we)|I take that back|disregard|ignore what I|that's incorrect|I need to fix|I messed up|wrong about|shouldn't have said|let me clarify|to clarify|I should say|more accurately|to be precise|strike that|that came out wrong|I didn't mean)\b") {
        for cap in re.find_iter(text) {
            results.corrections.push(extract_context(text, cap.start(), cap.end(), 50, 100));
        }
    }
    
    // â”€â”€ DECISIONS: Choices, commitments, direction changes â”€â”€
    // Discourse markers: conclusive connectors, resolution signals
    if let Ok(re) = Regex::new(r"(?i)\b(let's do|let's go with|decided to|go with|the plan is|we should|hold off|choosing|I'll go with|we'll use|switching to|committed to|settled on|final answer|the approach is|sticking with|opting for|I prefer|better to|the move is|I vote for|let's try|let's use|let's keep|let's drop|let's skip|let's avoid|ruling out|in favor of|over .{1,20} because|the winner is|going forward|from now on|new rule|new policy|the standard is)\b") {
        for cap in re.find_iter(text) {
            results.decisions.push(extract_context(text, cap.start(), cap.end(), 30, 80));
        }
    }
    
    // â”€â”€ DISCOVERIES: Insights, lessons, root causes â”€â”€
    // Discourse markers: evidential markers, epistemic shift signals
    if let Ok(re) = Regex::new(r"(?i)\b(turns out|realized|found out|the fix was|lesson learned|key insight|the trick is|the problem was|root cause|figured out|breakthrough|discovered|it works because|the issue was|the reason is|mystery solved|now I understand|makes sense now|the answer is|that explains|so that's why|the secret is|pro tip|TIL|learned that|good to know|worth noting|important detail|the catch is|gotcha:|caveat:|heads up|for future reference|note to self|remember that|the pattern is|takeaway)\b") {
        for cap in re.find_iter(text) {
            results.discoveries.push(extract_context(text, cap.start(), cap.end(), 40, 100));
        }
    }
    
    // â”€â”€ SUCCESS: Confirmations, wins, things that worked â”€â”€
    // AFINN positive valence words adapted for task context
    if let Ok(re) = Regex::new(r"(?i)\b(working now|fixed it|nailed it|that did it|solved|confirmed|verified|passing|deployed|shipped|looks good|works perfectly|success|brilliant|excellent|spot on|exactly right|perfect|love it|great job|nice work|well done|crushed it|clean build|all green|tests pass|no errors|smooth|flawless|mission accomplished|that's the one|bingo|jackpot|finally works|boom|ship it|done and done|good to go|ready to roll|locked in)\b") {
        for cap in re.find_iter(text) {
            results.success.push(extract_context(text, cap.start(), cap.end(), 30, 60));
        }
    }
    
    // â”€â”€ FRUSTRATION: Pain points, negative signals â”€â”€
    // AFINN negative valence + discourse frustration markers
    if let Ok(re) = Regex::new(r"(?i)\b(frustrated|annoying|ugh|keeps breaking|why does|still broken|doesn't work|gave up|wasted time|ridiculous|terrible|horrible|awful|broken again|this sucks|hate this|pain in|nightmare|headache|infuriating|maddening|unbelievable|seriously\?|come on|for the .{1,10} time|how is this|makes no sense|what the hell|waste of|killing me|driving me crazy|so slow|unacceptable|deal ?breaker|show ?stopper|blocker|regression|degraded|worse than|downgrade)\b") {
        for cap in re.find_iter(text) {
            results.frustration.push(extract_context(text, cap.start(), cap.end(), 40, 80));
        }
    }
    
    // â”€â”€ ANTI-PATTERNS: Recognized mistakes, things to avoid â”€â”€
    // Retrospective discourse markers, counterfactual reasoning
    if let Ok(re) = Regex::new(r"(?i)\b(should have|shouldn't have|next time|avoid|wrong approach|mistake was|don't do|bad idea|overkill|over.?engineered|premature|unnecessary|redundant|could have just|if only|in hindsight|looking back|that was dumb|lesson:|anti.?pattern|never again|the hard way|wasted effort|wrong call|missed the|overlooked|forgot to|failed to|neglected|skipped|shortcut that|hack that|technical debt|band.?aid|duct tape|workaround that|too complex|too clever|too many|too much)\b") {
        for cap in re.find_iter(text) {
            results.anti_patterns.push(extract_context(text, cap.start(), cap.end(), 40, 80));
        }
    }
    
    // â”€â”€ PIVOTS: Topic changes, transition markers â”€â”€
    // Discourse markers: topic-shift signals, boundary markers
    if let Ok(re) = Regex::new(r"(?i)\b(anyway|moving on|let's switch|back to|on another note|by the way|speaking of|tangent|sidebar|before I forget|also|oh and|one more thing|unrelated|separate topic|different question|quick question|while I have you|while we're at it|another thing|real quick|btw|shifting gears|circling back|returning to|as for|regarding|about that|now about|next up|on to)\b") {
        for cap in re.find_iter(text) {
            results.pivots.push(extract_context(text, cap.start(), cap.end(), 20, 60));
        }
    }
    
    // â”€â”€ HEDGING: Uncertainty, tentativeness â”€â”€
    // Discourse markers: epistemic hedges, modal weakeners
    if let Ok(re) = Regex::new(r"(?i)\b(I think|maybe|probably|not sure|might be|could be|possibly|it seems|appears to|I believe|I suppose|I guess|not certain|uncertain|unclear|hard to say|don't know if|I wonder|questionable|debatable|risky|tricky|complicated|iffy|sketchy|I should note|to be fair|part of me|on the other hand|then again|that said|having said that|I'm torn|not convinced|remains to be seen|TBD|open question)\b") {
        for cap in re.find_iter(text) {
            results.hedging.push(extract_context(text, cap.start(), cap.end(), 20, 80));
        }
    }
    
    // Build legacy aliases for backward compatibility
    results.emotions = results.frustration.iter()
        .chain(results.success.iter())
        .cloned()
        .collect();
    results.held_back = results.hedging.clone();
    
    // Score: weighted by extraction priority
    let weighted = (results.corrections.len() as f32 * 3.0)  // Tier 0
        + (results.decisions.len() as f32 * 3.0)              // Tier 0
        + (results.discoveries.len() as f32 * 3.0)            // Tier 0
        + (results.anti_patterns.len() as f32 * 2.0)          // Tier 1
        + (results.success.len() as f32 * 1.5)                // Tier 1
        + (results.frustration.len() as f32 * 1.5)            // Tier 1
        + (results.pivots.len() as f32 * 1.0)                 // Tier 2
        + (results.hedging.len() as f32 * 0.5);               // Low priority
    results.score = (weighted / 30.0).min(1.0);
    
    results
}

// ============================================================================
// Joe Fitness Function (Response Quality Scoring)
// ============================================================================

fn score_joe_fitness(response: &str) -> Value {
    let mut deductions: Vec<(&str, i32)> = Vec::new();
    let mut bonuses: Vec<(&str, i32)> = Vec::new();
    let text_lower = response.to_lowercase();
    
    // DEDUCTIONS
    // Hedge when direct (-1)
    let hedge_re = Regex::new(r"(?i)(it might be|perhaps|possibly|it's possible that|it could be|maybe|I think|I believe|in my opinion)").unwrap();
    let hedge_count = hedge_re.find_iter(response).count();
    if hedge_count > 2 {
        deductions.push(("Excessive hedging", -1 * (hedge_count as i32 - 1).min(3)));
    }
    
    // Ask permission for obvious (-2)
    let permission_re = Regex::new(r"(?i)(would you like me to|shall I|do you want me to|should I proceed|let me know if you'd like)").unwrap();
    if permission_re.is_match(response) {
        deductions.push(("Asking permission for obvious actions", -2));
    }
    
    // Explain vs just do (-1)
    if text_lower.contains("here's how") || text_lower.contains("first, let me explain") {
        if response.len() > 500 && !response.contains("```") {
            deductions.push(("Explaining instead of doing", -1));
        }
    }
    
    // Miss something obvious (-2) - hard to detect without context
    
    // Incomplete requiring follow-up (-1)
    if response.ends_with("?") && !response.contains("```") {
        let questions = response.matches('?').count();
        if questions > 2 {
            deductions.push(("Multiple questions instead of action", -1));
        }
    }
    
    // Shortcuts that bite later (-3)
    if text_lower.contains("should work") || text_lower.contains("common practice") {
        deductions.push(("Shortcut language detected", -3));
    }
    
    // BONUSES
    // Anticipated next question (+1)
    if text_lower.contains("you might also") || text_lower.contains("related:") || text_lower.contains("next step") {
        bonuses.push(("Anticipated follow-up", 1));
    }
    
    // Offered unwanted-but-useful (+2)
    if text_lower.contains("side note:") || text_lower.contains("fyi:") || text_lower.contains("heads up:") {
        bonuses.push(("Proactive information", 2));
    }
    
    // Caught own mistake (+1)
    if text_lower.contains("actually, let me correct") || text_lower.contains("wait, that's wrong") {
        bonuses.push(("Self-correction", 1));
    }
    
    // Elegant not just functional (+1)
    if response.contains("```") && response.len() < 800 {
        bonuses.push(("Concise with code", 1));
    }
    
    // Calculate total
    let deduction_total: i32 = deductions.iter().map(|(_, v)| v).sum();
    let bonus_total: i32 = bonuses.iter().map(|(_, v)| v).sum();
    let total = bonus_total + deduction_total; // deductions are negative
    
    let grade = match total {
        t if t >= 3 => "ðŸŒŸ Excellent",
        t if t >= 1 => "âœ… Good", 
        t if t >= -1 => "âš ï¸ Acceptable",
        _ => "âŒ Needs work"
    };
    
    json!({
        "score": total,
        "grade": grade,
        "deductions": deductions.iter().map(|(r, s)| json!({"reason": r, "points": s})).collect::<Vec<_>>(),
        "bonuses": bonuses.iter().map(|(r, s)| json!({"reason": r, "points": s})).collect::<Vec<_>>(),
        "summary": format!("{} ({}{})", grade, if total >= 0 { "+" } else { "" }, total)
    })
}

// ============================================================================
// Ollama Client
// ============================================================================

struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            base_url: OLLAMA_URL.to_string(),
        }
    }

    async fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>> {
        let resp = self.client
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&EmbedRequest {
                model: model.to_string(),
                prompt: text.to_string(),
            })
            .send()
            .await?
            .json::<EmbedResponse>()
            .await?;
        Ok(resp.embedding)
    }

    async fn generate(&self, model: &str, prompt: &str) -> Result<String> {
        let resp = self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&GenerateRequest {
                model: model.to_string(),
                prompt: prompt.to_string(),
                stream: false,
                keep_alive: Some("1h".to_string()),
            })
            .send()
            .await?
            .json::<GenerateResponse>()
            .await?;
        Ok(resp.response)
    }

    async fn fetch_url(&self, url: &str) -> Result<(String, u16)> {
        let resp = self.client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await?;
        Ok((text, status))
    }

    async fn list_models(&self) -> Result<Vec<Value>> {
        let resp = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .json::<OllamaTagsResponse>()
            .await?;
        Ok(resp.models.iter().map(|m| {
            let size_mb = m.size.map(|s| s / 1_000_000);
            json!({
                "name": m.name,
                "size_mb": size_mb,
                "family": m.details.as_ref().and_then(|d| d.family.as_deref()),
                "parameter_size": m.details.as_ref().and_then(|d| d.parameter_size.as_deref()),
                "quantization": m.details.as_ref().and_then(|d| d.quantization_level.as_deref()),
            })
        }).collect())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
    dot / (norm_a * norm_b)
}

// ============================================================================
// Server State
// ============================================================================

struct ServerState {
    ollama: OllamaClient,
    patterns: RwLock<HashMap<String, Vec<f32>>>,
}

impl ServerState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            ollama: OllamaClient::new(),
            patterns: RwLock::new(HashMap::new()),
        })
    }
}

// ============================================================================
// Tool Definitions
// ============================================================================

fn get_tool_definitions() -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "heuristics",
            "description": "Run heuristic pattern matching - finds corrections, decisions, discoveries, success, frustration, anti-patterns, pivots, hedging (instant, no API)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to analyze" }
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "embed",
            "description": "Get embedding vector for text via any Ollama embedding model. Caller specifies model.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to embed" },
                    "model": { "type": "string", "description": "Ollama embedding model to use (e.g. 'nomic-embed-text', 'mxbai-embed-large')" }
                },
                "required": ["text", "model"]
            }
        }),
        json!({
            "name": "store_pattern",
            "description": "Store a labeled pattern embedding for later comparison via any Ollama embedding model",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "label": { "type": "string", "description": "Label for this pattern" },
                    "text": { "type": "string", "description": "Example text for pattern" },
                    "model": { "type": "string", "description": "Ollama embedding model to use" }
                },
                "required": ["label", "text", "model"]
            }
        }),
        json!({
            "name": "compare",
            "description": "Compare text against stored patterns, returns similarity scores. Use the same model as store_pattern.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to compare" },
                    "model": { "type": "string", "description": "Ollama embedding model to use (should match model used in store_pattern)" }
                },
                "required": ["text", "model"]
            }
        }),
        json!({
            "name": "analyze",
            "description": "Deep retrospection analysis using a local Ollama generation model — thorough but slower",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Transcript or text to analyze" },
                    "model": { "type": "string", "description": "Ollama generation model to use (e.g. 'mistral:7b', 'llama3.2')" },
                    "focus": { "type": "string", "description": "Focus area: corrections, decisions, emotions, authenticity" }
                },
                "required": ["text", "model"]
            }
        }),
        json!({
            "name": "smart_fetch",
            "description": "Fetch URL, strip HTML, summarize with a local Ollama model. Saves 90%+ tokens. INSTANT currency check warns if content needs web_search verification.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "model": { "type": "string", "description": "Ollama generation model to use for summarization (e.g. 'mistral:7b', 'llama3.2')" },
                    "focus": { "type": "string", "description": "What to extract: 'main points', 'prices', 'contact info', etc." },
                    "max_tokens": { "type": "integer", "description": "Max summary length (default 500)", "default": 500 },
                    "include_raw": { "type": "boolean", "description": "Force include first 1000 chars (default: only on warnings)", "default": false },
                    "skip_summary": { "type": "boolean", "description": "Just fetch and clean HTML, skip LLM summarization (instant)", "default": false },
                    "timeout_secs": { "type": "integer", "description": "LLM timeout in seconds (default 60)", "default": 60 }
                },
                "required": ["url", "model"]
            }
        }),
        json!({
            "name": "health",
            "description": "Check Ollama reachability and list all available pulled models with metadata. No specific model assumed.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "score_response",
            "description": "Score a Claude response against Joe's fitness function. Returns breakdown of deductions/bonuses.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "response": { "type": "string", "description": "The response text to score" },
                    "context": { "type": "string", "description": "Optional: the user query for context" }
                },
                "required": ["response"]
            }
        }),
        // === OPS TOOLS (absorbed from ops server) ===
        json!({
            "name": "server_health",
            "description": "Check which MCP servers are alive. Returns process status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "servers": { "type": "array", "items": {"type": "string"}, "description": "Specific servers to check (default: all)" }
                }
            }
        }),
        json!({
            "name": "mcp_rebuild",
            "description": "Rebuild an MCP server with backup. Backs up exe, kills process, runs cargo build.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Server name (e.g., 'local', 'atlas', 'utonomous')" }
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "error_fallbacks",
            "description": "Query learned error fallback patterns. Returns known error-to-fallback mappings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "error_pattern": { "type": "string", "description": "Error text to match (optional, omit to list all)" }
                }
            }
        }),
    ];
    // Append semantic search tools (from semantic.rs module)
    tools.extend(semantic::tool_definitions());
    tools.push(planner::get_definition());
    tools
}


// ============================================================================
// Tool Execution
// ============================================================================

async fn execute_tool(state: &Arc<ServerState>, name: &str, args: Value) -> Result<Value> {
    match name {
        "heuristics" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let results = run_heuristics(text);
            Ok(json!(results))
        }
        
        "embed" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let model = args.get("model").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
            let embedding = state.ollama.embed(model, text).await?;
            Ok(json!({
                "dimensions": embedding.len(),
                "first_5": &embedding[..5.min(embedding.len())],
                "model": model,
                "success": true
            }))
        }

        "store_pattern" => {
            let label = args.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let model = args.get("model").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
            let embedding = state.ollama.embed(model, text).await?;

            let count = {
                let mut patterns = state.patterns.write().unwrap();
                patterns.insert(label.clone(), embedding);
                patterns.len()
            };

            Ok(json!({
                "success": true,
                "label": label,
                "model": model,
                "total_patterns": count
            }))
        }

        "compare" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let model = args.get("model").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
            let embedding = state.ollama.embed(model, text).await?;

            let patterns = state.patterns.read().unwrap();
            if patterns.is_empty() {
                return Ok(json!({
                    "success": false,
                    "error": "No patterns stored. Use store_pattern first."
                }));
            }

            let mut scores: Vec<(String, f32)> = patterns.iter()
                .map(|(label, emb)| (label.clone(), cosine_similarity(&embedding, emb)))
                .collect();
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            Ok(json!({
                "success": true,
                "model": model,
                "matches": scores.into_iter()
                    .map(|(label, score)| json!({"label": label, "similarity": score}))
                    .collect::<Vec<_>>()
            }))
        }
        
        "analyze" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let model = args.get("model").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
            let focus = args.get("focus").and_then(|v| v.as_str()).unwrap_or("general retrospection");

            let heuristics = run_heuristics(text);

            let prompt = format!(r#"You are analyzing a conversation transcript for retrospective self-understanding.

Heuristic pre-analysis found:
- {} corrections, {} decisions, {} discoveries
- {} successes, {} frustrations, {} anti-patterns
- {} pivots, {} hedging moments

Focus: {}

Analyze the following text and provide:
1. Key patterns observed
2. Confidence gaps (overstatements)
3. Authenticity assessment
4. Recommendations for next instance

TEXT:
{}

Provide concise, specific analysis."#,
                heuristics.corrections.len(),
                heuristics.decisions.len(),
                heuristics.discoveries.len(),
                heuristics.success.len(),
                heuristics.frustration.len(),
                heuristics.anti_patterns.len(),
                heuristics.pivots.len(),
                heuristics.hedging.len(),
                focus,
                &text[..text.len().min(4000)]
            );

            let analysis = state.ollama.generate(model, &prompt).await?;

            Ok(json!({
                "success": true,
                "model": model,
                "heuristics": heuristics,
                "analysis": analysis
            }))
        }
        
        "smart_fetch" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let model = args.get("model").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
            let focus = args.get("focus").and_then(|v| v.as_str()).unwrap_or("main points and key information");
            let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let force_include_raw = args.get("include_raw").and_then(|v| v.as_bool()).unwrap_or(false);
            let skip_summary = args.get("skip_summary").and_then(|v| v.as_bool()).unwrap_or(false);
            let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(60) as u64;
            
            let mut warnings: Vec<String> = vec![];
            
            // Fetch URL
            let fetch_start = Instant::now();
            let (raw_html, status) = match state.ollama.fetch_url(url).await {
                Ok(result) => result,
                Err(e) => {
                    return Ok(json!({
                        "success": false,
                        "error": format!("Fetch failed: {}. Try web_fetch as fallback.", e),
                        "url": url
                    }));
                }
            };
            let fetch_time_ms = fetch_start.elapsed().as_millis();
            
            if status != 200 {
                warnings.push(format!("HTTP status {}", status));
            }
            
            let source_length = raw_html.len();
            
            // Strip HTML
            let clean_text = strip_html(&raw_html);
            let clean_length = clean_text.len();
            
            // INSTANT: Check for time-sensitive content (no Mistral, ~1ms)
            let currency_check = check_currency_needs(&clean_text);
            
            // Check for JS-heavy / empty content
            if clean_length < 200 {
                warnings.push("Very little text extracted - page may be JS-rendered. Try browser:http_scrape".to_string());
            }
            
            // If skip_summary, return cleaned text only
            if skip_summary {
                let (preview, truncated) = truncate_to_chars(&clean_text, 2000);
                if truncated {
                    warnings.push("Text truncated to 2000 chars".to_string());
                }
                let mut response = json!({
                    "success": true,
                    "mode": "clean_only",
                    "text": preview,
                    "source_length": source_length,
                    "clean_length": clean_length,
                    "fetch_time_ms": fetch_time_ms,
                    "warnings": warnings
                });
                if currency_check.needs_verification {
                    response["currency_warning"] = json!({
                        "needs_verification": true,
                        "reasons": currency_check.reasons,
                        "action": "Use web_search to verify current info before relying on this"
                    });
                }
                return Ok(response);
            }
            
            // Truncate for Mistral (safe limit ~16K chars = ~4K tokens)
            let max_input_chars = 16000;
            let (input_text, was_truncated) = truncate_to_chars(&clean_text, max_input_chars);
            if was_truncated {
                warnings.push(format!("Input truncated from {} to {} chars for LLM", clean_length, max_input_chars));
            }
            
            // Build summary prompt
            let max_words = max_tokens * 3 / 4;
            let prompt = format!(r#"Summarize the following web page content.

FOCUS: {}
MAX LENGTH: {} words

RULES:
1. Extract the most important information related to the focus
2. Include specific facts, numbers, names when relevant
3. If the content seems unrelated to the focus, summarize what IS there
4. Include 2-3 direct quotes that support key points (mark with "...")

CONTENT:
{}

SUMMARY:"#, focus, max_words, input_text);
            
            // Generate summary with timeout
            let summary_start = Instant::now();
            let summary_future = state.ollama.generate(model, &prompt);
            let summary = match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs), 
                summary_future
            ).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    // LLM error - return cleaned text
                    let (preview, _) = truncate_to_chars(&clean_text, 1500);
                    return Ok(json!({
                        "success": false,
                        "error": format!("LLM failed: {}", e),
                        "model": model,
                        "fallback_text": preview,
                        "source_length": source_length,
                        "clean_length": clean_length,
                        "fetch_time_ms": fetch_time_ms,
                        "warnings": warnings,
                        "currency_check": currency_check
                    }));
                }
                Err(_) => {
                    // Timeout
                    warnings.push(format!("LLM timed out after {}s", timeout_secs));
                    let (preview, _) = truncate_to_chars(&clean_text, 1500);
                    return Ok(json!({
                        "success": false,
                        "error": format!("Timeout after {}s", timeout_secs),
                        "fallback_text": preview,
                        "source_length": source_length,
                        "clean_length": clean_length,
                        "fetch_time_ms": fetch_time_ms,
                        "warnings": warnings,
                        "hint": "For faster results, use skip_summary:true or web_search for current info"
                    }));
                }
            };
            let summary_time_ms = summary_start.elapsed().as_millis();
            
            // Build response - only include raw if forced OR warnings exist
            let include_raw = force_include_raw || !warnings.is_empty() || currency_check.needs_verification;
            
            let mut response = json!({
                "success": true,
                "summary": summary.trim(),
                "source_length": source_length,
                "clean_length": clean_length,
                "summary_length": summary.len(),
                "compression": format!("{:.0}%", (1.0 - (summary.len() as f64 / source_length as f64)) * 100.0),
                "fetch_time_ms": fetch_time_ms,
                "summary_time_ms": summary_time_ms
            });
            
            // Add warnings if any
            if !warnings.is_empty() {
                response["warnings"] = json!(warnings);
            }
            
            // Add currency warning if detected
            if currency_check.needs_verification {
                response["currency_warning"] = json!({
                    "needs_verification": true,
                    "reasons": currency_check.reasons,
                    "matches": currency_check.time_sensitive_matches,
                    "action": "Use web_search to verify current info before relying on this summary"
                });
            }
            
            // Include raw preview only when needed
            if include_raw {
                let (preview, _) = truncate_to_chars(&clean_text, 1000);
                response["raw_preview"] = json!(preview);
            }
            
            Ok(response)
        }
        
        "health" => {
            match state.ollama.list_models().await {
                Ok(models) => Ok(json!({
                    "success": true,
                    "ollama_reachable": true,
                    "model_count": models.len(),
                    "models": models,
                    "note": "No specific model assumed. Specify model per tool call."
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "ollama_reachable": false,
                    "error": format!("Ollama not responding: {}. Is 'ollama serve' running?", e)
                }))
            }
        }
        
        "score_response" => {
            let response = args.get("response").and_then(|v| v.as_str()).unwrap_or("");
            let _context = args.get("context").and_then(|v| v.as_str()).unwrap_or("");
            let result = score_joe_fitness(response);
            Ok(result)
        }
        
        "semantic_search" => {
            semantic::handle_semantic_search(&args).await
        }
        "semantic_reindex" => {
            semantic::handle_semantic_reindex(&args).await
        }
        // === OPS TOOLS ===
        "server_health" => {
            let map_content = std::fs::read_to_string(FALLBACK_MAP_PATH)
                .unwrap_or_else(|_| "{}".to_string());
            let map: Value = serde_json::from_str(&map_content).unwrap_or(json!({}));
            let filter: Option<Vec<String>> = args.get("servers")
                .and_then(|s| serde_json::from_value(s.clone()).ok());
            let servers = map.get("servers").and_then(|s| s.as_object());
            
            if let Some(servers) = servers {
                let mut results = serde_json::Map::new();
                let mut alive = 0u32;
                let mut dead = 0u32;
                for (name, config) in servers {
                    if let Some(ref f) = filter {
                        if !f.iter().any(|s| s == name) { continue; }
                    }
                    let process = config.get("process").and_then(|p| p.as_str()).unwrap_or("unknown");
                    let is_alive = is_process_running(process);
                    if is_alive { alive += 1; } else { dead += 1; }
                    let mirror = config.get("mirror").and_then(|m| m.as_str()).unwrap_or("none");
                    let critical = config.get("critical").and_then(|c| c.as_bool()).unwrap_or(false);
                    results.insert(name.clone(), json!({
                        "alive": is_alive, "process": process, "mirror": mirror, "critical": critical
                    }));
                }
                Ok(json!({ "servers": results, "summary": { "alive": alive, "dead": dead, "total": alive + dead } }))
            } else {
                Ok(json!({"error": "No servers in fallback map", "path": FALLBACK_MAP_PATH}))
            }
        }
        
        "mcp_rebuild" => {
            use std::process::Command as Cmd;
            let target = args.get("target").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'target' parameter"))?;
            let rust_dir = std::path::PathBuf::from(r"C:\rust-mcp");
            let target_dir = rust_dir.join(target);
            let exe_name = format!("{}.exe", target);
            let exe_path = rust_dir.join("target").join("release").join(&exe_name);
            let backup_dir = rust_dir.join("backups");
            
            if !target_dir.exists() {
                return Ok(json!({"error": format!("Target '{}' not found at {:?}", target, target_dir)}));
            }
            std::fs::create_dir_all(&backup_dir).ok();
            
            // Backup
            let backup_path = if exe_path.exists() {
                let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                let bp = backup_dir.join(format!("{}_{}.exe", target, epoch));
                std::fs::copy(&exe_path, &bp).ok();
                Some(bp.display().to_string())
            } else { None };
            
            // Kill
            let killed = Cmd::new("taskkill").args(["/F", "/IM", &exe_name]).output()
                .map(|o| o.status.success()).unwrap_or(false);
            std::thread::sleep(std::time::Duration::from_secs(3));
            
            // Build
            let cargo = std::env::var("USERPROFILE").map(|p| std::path::PathBuf::from(p).join(".cargo").join("bin").join("cargo.exe")).unwrap_or_else(|_| std::path::PathBuf::from("cargo"));
            let build = Cmd::new(&cargo).args(["build", "--release"]).current_dir(&target_dir)
                .output().map_err(|e| anyhow::anyhow!("Cargo failed: {}", e))?;
            let success = build.status.success();
            let stderr = String::from_utf8_lossy(&build.stderr);
            
            Ok(json!({
                "target": target,
                "backup_path": backup_path,
                "process_killed": killed,
                "build_success": success,
                "new_exe_exists": exe_path.exists(),
                "build_stderr_preview": stderr.chars().take(500).collect::<String>(),
                "message": if success { format!("Rebuilt {}. Restart Claude.", target) }
                    else { format!("Build failed for {}", target) }
            }))
        }
        
        "error_fallbacks" => {
            let fallbacks_content = std::fs::read_to_string(ERROR_FALLBACKS_PATH)
                .unwrap_or_else(|_| "{}".to_string());
            let fallbacks: Value = serde_json::from_str(&fallbacks_content).unwrap_or(json!({}));
            
            if let Some(pattern) = args.get("error_pattern").and_then(|v| v.as_str()) {
                let pattern_lower = pattern.to_lowercase();
                let mut matches = Vec::new();
                if let Some(obj) = fallbacks.as_object() {
                    for (key, val) in obj {
                        let symptom = val.get("symptom").and_then(|s| s.as_str()).unwrap_or("");
                        if symptom.to_lowercase().contains(&pattern_lower) || key.to_lowercase().contains(&pattern_lower) {
                            matches.push(json!({"name": key, "config": val}));
                        }
                    }
                }
                Ok(json!({"matches": matches, "total_patterns": fallbacks.as_object().map(|o| o.len()).unwrap_or(0)}))
            } else {
                Ok(json!({"fallbacks": fallbacks, "total": fallbacks.as_object().map(|o| o.len()).unwrap_or(0)}))
            }
        }
        
        "plan" => Ok(planner::plan(&args)),
        "assemble" => Ok(planner::assemble(&args)),
            _ => Ok(json!({"error": format!("Unknown tool: {}", name)}))
    }
}

// ============================================================================
// JSON-RPC Protocol
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

async fn handle_request(state: &Arc<ServerState>, method: &str, id: Value, params: Option<Value>) -> JsonRpcResponse {
    match method {
        "initialize" => {
            info!("Initialize");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "echo",
                        "version": "1.2.0"
                    }
                })),
                error: None,
            }
        }
        
        "tools/list" => {
            info!("Tools list");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({ "tools": get_tool_definitions() })),
                error: None,
            }
        }
        
        "tools/call" => {
            let params = params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let tool_args = params.get("arguments").cloned().unwrap_or(json!({}));
            
            info!("Tool call: {}", tool_name);
            
            match execute_tool(state, tool_name, tool_args).await {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                        }]
                    })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("{{\"error\": \"{}\"}}", e)
                        }],
                        "isError": true
                    })),
                    error: None,
                }
            }
        }
        
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        },
        
        _ => {
            warn!("Unknown method: {}", method);
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", method),
                }),
            }
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("Retrospector MCP v1.2.0 starting (with smart_fetch + currency detection)...");
    
    let state = ServerState::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("Read error: {}", e);
                continue;
            }
        };
        
        if line.trim().is_empty() { continue; }
        
        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        };

        // Validate JSON-RPC 2.0 version
        if let Some(ref version) = request.jsonrpc {
            if version != "2.0" {
                warn!("Invalid JSON-RPC version: {}", version);
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone().unwrap_or(Value::Null),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32600,
                        message: format!("Invalid JSON-RPC version: expected '2.0', got '{}'", version),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        }

        let method = match &request.method {
            Some(m) => m.clone(),
            None => continue,
        };
        
        if request.id.is_none() || method.starts_with("notifications/") {
            continue;
        }
        
        let response = handle_request(&state, &method, request.id.unwrap_or(Value::Null), request.params).await;
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    
    Ok(())
}

fn is_process_running(name: &str) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", name), "/NH"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(name),
        Err(_) => false,
    }
}

// === FILE NAVIGATION ===
// Generated: 2026-02-13T11:32:56
// Total: 1079 lines | 11 functions | 11 structs | 3 constants
//
// IMPORTS: anyhow, regex, reqwest, serde, serde_json, std, tracing, tracing_subscriber
//
// CONSTANTS:
//   const OLLAMA_URL: 27
//   const EMBED_MODEL: 28
//   const ANALYSIS_MODEL: 29
//
// STRUCTS:
//   EmbedRequest: 36-39
//   EmbedResponse: 42-44
//   GenerateRequest: 47-52
//   GenerateResponse: 55-57
//   CurrencyCheck: 118-122
//   HeuristicResults: 167-180
//   OllamaClient: 371-374
//   ServerState: 442-445
//   JsonRpcRequest: 888-894
//   JsonRpcResponse: 897-904
//   JsonRpcError: 907-910
//
// IMPL BLOCKS:
//   impl OllamaClient: 376-427
//   impl ServerState: 447-454
//
// FUNCTIONS:
//   strip_html: 63-98
//   truncate_to_chars: 100-111
//   check_currency_needs: 124-158
//   extract_context: 182-189
//   run_heuristics: 191-278 [med]
//   score_joe_fitness: 284-365 [med]
//   cosine_similarity: 429-436
//   get_tool_definitions: 460-559 [med]
//   execute_tool: 565-881 [LARGE]
//   handle_request: 912-995 [med]
//   main: 1002-1079 [med]
//
// === END FILE NAVIGATION ===
