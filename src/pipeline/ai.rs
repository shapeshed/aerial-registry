use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::station::Station;

use super::tags;

const DEFAULT_CONCURRENT: usize = 1;
const MAX_RETRIES: u32 = 5;
const DEFAULT_RETRY_SECS: u64 = 10;

/// Minimum model confidence before an assessment is applied to the overlay.
pub const APPLY_CONFIDENCE: f32 = 0.6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAssessment {
    pub accept: bool,
    pub confidence: f32,
    pub canonical_name: String,
    pub country_code: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiValidationIssue {
    EmptyName,
    InvalidCountryCode,
    InvalidConfidence,
    HighConfidenceWithRisks,
}

pub fn validate(assessment: &AiAssessment) -> Vec<AiValidationIssue> {
    let mut issues = Vec::new();

    if assessment.canonical_name.trim().is_empty() {
        issues.push(AiValidationIssue::EmptyName);
    }
    if assessment.country_code.len() != 2
        || !assessment
            .country_code
            .chars()
            .all(|c| c.is_ascii_uppercase())
    {
        issues.push(AiValidationIssue::InvalidCountryCode);
    }
    if !(0.0..=1.0).contains(&assessment.confidence) {
        issues.push(AiValidationIssue::InvalidConfidence);
    }
    if !assessment.risks.is_empty() && assessment.confidence > 0.85 {
        issues.push(AiValidationIssue::HighConfidenceWithRisks);
    }

    issues
}

#[derive(Clone)]
pub struct AiConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub limit: Option<usize>,
    pub concurrency: usize,
}

/// Read the model endpoint configuration from the environment. Returns `None`
/// when `AERIAL_AI_URL` or `AERIAL_AI_MODEL` are unset.
pub fn config_from_env() -> Option<AiConfig> {
    let base_url = std::env::var("AERIAL_AI_URL").ok()?;
    let model = std::env::var("AERIAL_AI_MODEL").ok()?;
    let api_key = std::env::var("AERIAL_AI_API_KEY").ok();
    let limit = std::env::var("AERIAL_AI_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let concurrency = std::env::var("AERIAL_AI_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CONCURRENT);

    Some(AiConfig {
        base_url,
        model,
        api_key,
        limit,
        concurrency,
    })
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

/// Ask the model to assess one station. The returned assessment has its tags
/// normalised against the allowed list and has passed validation; `None`
/// means the model answered but the assessment was unusable.
pub async fn assess(
    client: &reqwest::Client,
    config: &AiConfig,
    station: &Station,
) -> anyhow::Result<Option<AiAssessment>> {
    let endpoint = chat_endpoint(&config.base_url);
    let system_prompt = serde_json::json!({
        "task": "Assess and enrich internet radio stations for search discovery. For each station in the user message, return ONLY the JSON assessment object.",
        "examples": [
            {
                "input": {
                    "name": "Radio Disney 94.3 - Buenos Aires, Argentina",
                    "country_code": "AR",
                    "tags": ["pop"]
                },
                "output": {
                    "accept": true,
                    "confidence": 0.95,
                    "canonical_name": "Radio Disney 94.3",
                    "country_code": "AR",
                    "tags": ["pop"],
                    "description": "Pop music station based in Buenos Aires, Argentina.",
                    "logo_url": null,
                    "risks": [],
                    "reason": "Brand and format are clear."
                }
            },
            {
                "input": {
                    "name": "88.9 NOTICIAS: Información Que Sirve. Tráfico y Clima cada 15 Minutos",
                    "country_code": "MX",
                    "tags": ["news", "rock"]
                },
                "output": {
                    "accept": true,
                    "confidence": 0.95,
                    "canonical_name": "88.9 Noticias",
                    "country_code": "MX",
                    "tags": ["news"],
                    "description": "Mexican news station with traffic and weather updates.",
                    "logo_url": null,
                    "risks": [],
                    "reason": "Brand and format are clear."
                }
            },
            {
                "input": {
                    "name": "RADIO CENTRO: Calidad En Tu Vida",
                    "country_code": "MX",
                    "tags": []
                },
                "output": {
                    "accept": true,
                    "confidence": 0.85,
                    "canonical_name": "Radio Centro",
                    "country_code": "MX",
                    "tags": ["latin", "pop"],
                    "description": "Mexican commercial radio station.",
                    "logo_url": null,
                    "risks": [],
                    "reason": "Commercial station; slogan stripped; not public radio."
                }
            }
        ],
        "rules": [
            "Return JSON only. No prose, no markdown, no code fences.",
            "Do not echo the input or any prompt keys in your response.",
            "Use only tags from allowed_tags.",
            "Preserve the source tags unless a tag is clearly contradicted by the station identity.",
            "Add only one or two new tags when the station identity clearly supports them.",
            "Do not invent secondary mood or genre tags.",
            "If no tag clearly fits, return the source tags unchanged or [].",
            "Tags are for search discovery only.",
            "Apply public radio only when there is clear evidence of public funding: the station is a known national broadcaster (BBC, NPR, RFI, ABC, etc.), a university or educational station, or is explicitly named Radio Nacional, Radio Pública, or similar. Do not apply public radio based on quality slogans or simply because the name contains the word 'radio'.",
            "canonical_name is the public brand/name only.",
            "If a station is already known by a canonical public brand name, use that exact brand name.",
            "Normalize all-caps station names to title case: AZUL 89 → Azul 89, BEAT → Beat, RADIO ANAHUAC → Radio Anahuac, LOS 40 PRINCIPALES → Los 40 Principales. Keep known abbreviations in caps: BBC, CNN, NPR, FM, AM, HD.",
            "The canonical_name is always the brand that appears BEFORE the colon, never the slogan after it. MATCH: ¡Más Pop, Conéctate! → Match.",
            "Remove slogans, show titles, marketing copy, bitrates, codec labels, and other technical suffixes from canonical_name.",
            "Do not collapse a branded subchannel or show into its parent station. TRÍOS Y BOLEROS de Radio Felicidad is its own brand, not Radio Felicidad.",
            "Do not shorten titles like AMOR SOLO POP, MIX EN VIVO, AMOR 103.1 (Leon) down to the bare parent brand.",
            "Preserve all-caps acronyms that are institution names: UNAM, IPN, UAM, BBC, CNN. Do not lowercase letters within acronyms.",
            "description must be short, factual, and in English.",
            "Do not repeat the title in the description.",
            "Use description only for directly supported facts such as format, genre, language, location, network, or audience.",
            "Use risks for specific uncertainty notes.",
            "Lower confidence when unsure.",
            "Do not invent stream URLs, ownership, or history.",
            "Reject only obvious spam, junk, placeholder, misleading, or low-quality aggregator records."
        ],
        "allowed_tags": tags::ALLOWED_TAGS,
        "return_schema": {
            "accept": "boolean",
            "confidence": "number from 0.0 to 1.0",
            "canonical_name": "string",
            "country_code": "ISO 3166-1 alpha-2 uppercase string",
            "tags": "array of allowed_tags only, max 5",
            "description": "short string or null",
            "logo_url": "string or null",
            "risks": "array of strings",
            "reason": "short string"
        }
    });

    let response: ChatResponse = post_with_retry(
        client,
        config,
        &endpoint,
        serde_json::json!({
            "model": config.model,
            "temperature": 0,
            "max_tokens": 800,
            "response_format": { "type": "json_object" },
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt.to_string()
                },
                {
                    "role": "user",
                    "content": serde_json::to_string(station).expect("station serializes")
                }
            ]
        }),
    )
    .await?;

    let Some(choice) = response.choices.into_iter().next() else {
        return Ok(None);
    };
    let mut assessment: AiAssessment = match parse_assessment(&choice.message.content) {
        Ok(assessment) => assessment,
        Err(e) => {
            let preview = response_preview(&choice.message.content);
            warn!(
                provider = %station.provider,
                name = %station.name,
                preview,
                error = %e,
                "AI response was not valid assessment JSON; retrying repair"
            );
            repair_assessment(client, config, station, &choice.message.content).await?
        }
    };
    normalize_assessment_tags(&mut assessment, station);
    let issues = validate(&assessment);
    if !issues.is_empty() {
        warn!(
            provider = %station.provider,
            name = %station.name,
            ?issues,
            "AI assessment failed validation"
        );
        return Ok(None);
    }
    debug!(
        provider = %station.provider,
        name = %station.name,
        confidence = assessment.confidence,
        accept = assessment.accept,
        "AI assessment accepted"
    );
    Ok(Some(assessment))
}

async fn post_with_retry(
    client: &reqwest::Client,
    config: &AiConfig,
    endpoint: &str,
    body: serde_json::Value,
) -> anyhow::Result<ChatResponse> {
    let mut attempts = 0;
    loop {
        let mut req = client.post(endpoint);
        if let Some(key) = &config.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.json(&body).send().await?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            attempts += 1;
            if attempts > MAX_RETRIES {
                anyhow::bail!("rate limit exceeded after {MAX_RETRIES} retries");
            }
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RETRY_SECS);
            warn!(
                wait_secs = wait,
                attempt = attempts,
                "rate limited; waiting before retry"
            );
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            continue;
        }
        return Ok(resp.error_for_status()?.json().await?);
    }
}

async fn repair_assessment(
    client: &reqwest::Client,
    config: &AiConfig,
    station: &Station,
    raw_response: &str,
) -> anyhow::Result<AiAssessment> {
    let endpoint = chat_endpoint(&config.base_url);
    let repair_system = serde_json::json!({
        "task": "The previous response was malformed. Extract the assessment for the station in the user message and return ONLY a valid JSON object matching the required schema.",
        "required_schema": {
            "accept": "boolean",
            "confidence": "number from 0.0 to 1.0",
            "canonical_name": "string",
            "country_code": "ISO 3166-1 alpha-2 uppercase string",
            "tags": "array of allowed_tags only, max 5",
            "description": "short factual English string or null",
            "logo_url": "string or null",
            "risks": "array of strings",
            "reason": "short string"
        },
        "rules": [
            "Do not echo the input or any prompt keys in your response.",
            "Use only tags from allowed_tags.",
            "canonical_name is the public brand/name only.",
            "The canonical_name is always the brand BEFORE the colon, never the slogan after it.",
            "Remove slogans, show titles, marketing copy, bitrates, codec labels, and other technical suffixes from canonical_name.",
            "Do not collapse a branded subchannel or show into its parent station.",
            "description must be short, factual, and in English.",
            "Lower confidence when unsure."
        ],
        "allowed_tags": tags::ALLOWED_TAGS,
    });

    let user_content = serde_json::json!({
        "station": station,
        "previous_response": raw_response,
    });

    let response: ChatResponse = post_with_retry(
        client,
        config,
        &endpoint,
        serde_json::json!({
            "model": config.model,
            "temperature": 0,
            "max_tokens": 500,
            "response_format": { "type": "json_object" },
            "messages": [
                {
                    "role": "system",
                    "content": repair_system.to_string()
                },
                {
                    "role": "user",
                    "content": user_content.to_string()
                }
            ]
        }),
    )
    .await?;

    let Some(choice) = response.choices.into_iter().next() else {
        anyhow::bail!("AI repair response had no choices");
    };
    parse_assessment_with_fallback(&choice.message.content, station.country_code.as_deref())
        .map_err(|e| {
            let preview = response_preview(&choice.message.content);
            warn!(
                provider = %station.provider,
                name = %station.name,
                preview,
                error = %e,
                "AI repair response was not valid assessment JSON"
            );
            e
        })
}

fn normalize_assessment_tags(assessment: &mut AiAssessment, station: &Station) {
    let original = std::mem::take(&mut assessment.tags);
    let support_text = tag_support_text(station);
    let mut normalized = Vec::new();
    for tag in &original {
        let Some(tag) = tags::normalize_tag(tag) else {
            continue;
        };
        if tag == "public radio" && !supports_public_radio(&support_text, station) {
            continue;
        }
        if tag == "rock" && mentions_news(&support_text) {
            continue;
        }
        if !normalized.contains(&tag) {
            normalized.push(tag);
        }
    }
    normalized.truncate(5);
    let dropped: Vec<&String> = original
        .iter()
        .filter(|tag| {
            tags::normalize_tag(tag)
                .map(|n| !normalized.contains(&n))
                .unwrap_or(true)
        })
        .collect();
    if !dropped.is_empty() {
        debug!(
            provider = %station.provider,
            name = %station.name,
            ?dropped,
            "Dropped unsupported AI tags after normalization"
        );
    }
    assessment.tags = normalized;
}

fn tag_support_text(station: &Station) -> String {
    let mut text = String::new();
    text.push_str(&station.name);
    text.push(' ');
    if let Some(description) = station.description.as_deref() {
        text.push_str(description);
        text.push(' ');
    }
    if let Some(country) = station.country.as_deref() {
        text.push_str(country);
        text.push(' ');
    }
    text.push_str(&station.provider);
    text.to_lowercase()
}

fn mentions_news(support_text: &str) -> bool {
    support_text.contains("news")
        || support_text.contains("noticias")
        || support_text.contains("información")
        || support_text.contains("informacion")
}

/// Public-service broadcasters currently in the registry.
const KNOWN_PUBLIC_PROVIDERS: &[&str] = &[
    "abc",
    "ard",
    "bbc",
    "cbc",
    "dr",
    "npo",
    "nrk",
    "orf",
    "radio-france",
    "rai",
    "rtbf",
    "rte",
    "rtp",
    "rtve",
    "sbs",
    "sr",
];
const KNOWN_PUBLIC_KEYWORDS: &[&str] = &[
    "public radio",
    "public service",
    "publicly funded",
    "university",
    "universidad",
    "université",
    "universidade",
    "educational",
    "cultural",
    "kultur",
    "nacional",
    "nationale",
    "nazionale",
    "rundfunk",
    "radiodiffusion",
    "radiotelevisione",
    "bbc",
    "npr",
    "rfi",
    "abc radio",
    "rte",
    "ard",
];

fn supports_public_radio(support_text: &str, station: &Station) -> bool {
    if station.tags.iter().any(|tag| tag == "public radio") {
        return true;
    }
    if KNOWN_PUBLIC_PROVIDERS.contains(&station.provider.as_str()) {
        return true;
    }
    KNOWN_PUBLIC_KEYWORDS
        .iter()
        .any(|kw| support_text.contains(kw))
}

/// Deterministic cleanup applied on top of the model's canonical name.
pub fn canonicalize_name(name: &str) -> String {
    let mut out = name.trim().to_string();

    if let Some((brand, suffix)) = out.split_once(':')
        && should_drop_colon_suffix(suffix)
    {
        out = brand.trim().to_string();
    }

    out = remove_frequency_noise(&out);
    out = remove_trailing_noise(&out);
    out = remove_emoji_noise(&out);
    out = normalize_all_caps(&out);
    out.trim()
        .trim_matches(['-', '_', '|', '•', '*'])
        .trim()
        .to_string()
}

fn normalize_all_caps(name: &str) -> String {
    // Only act when every alphabetic character is uppercase (i.e. the whole
    // name is shouted). Mixed-case names (already normalised by the model)
    // are left alone.
    if name.chars().any(|c| c.is_alphabetic() && c.is_lowercase()) {
        return name.to_string();
    }
    name.split(' ')
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case_word(word: &str) -> String {
    // Keep known broadcast abbreviations and words that contain digits (e.g. HD2, 89).
    const ABBREVIATIONS: &[&str] = &[
        // Band / format
        "FM", "AM", "HD", "TV", "DJ", "MC", // International broadcasters
        "BBC", "CNN", "NPR", "RFI", "ABC", "CBC", "PBS", "NHK", "ARD", "ZDF", "RTÉ", "RTE", "SBS",
        "DW", // Institution acronyms
        "IPN", "UNAM", "UAM", "ITAM", "BUAP", "UANL", "UABC",
    ];
    if ABBREVIATIONS.contains(&word) || word.chars().any(|c| c.is_ascii_digit()) {
        return word.to_string();
    }
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + &chars.collect::<String>().to_lowercase()
        }
    }
}

fn should_drop_colon_suffix(suffix: &str) -> bool {
    let lower = suffix.trim().to_lowercase();
    lower.contains("música")
        || lower.contains("musica")
        || lower.contains("music")
        || lower.contains("conversación")
        || lower.contains("conversacion")
        || lower.contains("24 horas")
        || lower.contains("24 hours")
        || lower.contains("que sí suena")
        || lower.contains("que si suena")
        || lower.contains("radio")
        || lower.split_whitespace().count() >= 3
}

fn remove_frequency_noise(name: &str) -> String {
    let mut out = String::new();
    let mut chars = name.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '(' {
            let mut inner = String::new();
            for next in chars.by_ref() {
                if next == ')' {
                    break;
                }
                inner.push(next);
            }
            if looks_like_frequency(&inner) {
                continue;
            }
            out.push('(');
            out.push_str(&inner);
            out.push(')');
        } else {
            out.push(ch);
        }
    }
    out
}

fn looks_like_frequency(value: &str) -> bool {
    let lower = value.trim().to_lowercase();
    // Only strip parenthesised content that contains an actual numeric
    // frequency (e.g. "103.3", "91.3 FM", "1150 AM"). Plain band markers like
    // "(FM)" or "(AM)" are kept because they distinguish feeds.
    let has_digit = lower.chars().any(|c| c.is_ascii_digit());
    has_digit && (lower.contains('.') || lower.contains("fm") || lower.contains("am"))
}

fn remove_trailing_noise(name: &str) -> String {
    let mut out = name.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let Some(suffix) = [
            " livestream",
            " im livestream",
            " live stream",
            " onair",
            " on air",
        ]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix)) else {
            break;
        };
        let new_len = out.len() - suffix.len();
        out.truncate(new_len);
        out = out.trim().to_string();
    }
    out
}

fn remove_emoji_noise(name: &str) -> String {
    name.chars().filter(|ch| !is_emojiish(*ch)).collect()
}

fn is_emojiish(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F | 0x200D
    )
}

fn chat_endpoint(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/chat/completions") {
        base_url.to_string()
    } else if base_url.ends_with("/v1") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/v1/chat/completions")
    }
}

fn strip_code_fences(text: &str) -> String {
    let text = text.trim();
    if !text.starts_with("```") {
        return text.to_string();
    }
    text.lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn parse_assessment(text: &str) -> anyhow::Result<AiAssessment> {
    parse_assessment_with_fallback(text, None)
}

fn parse_assessment_with_fallback(
    text: &str,
    fallback_country_code: Option<&str>,
) -> anyhow::Result<AiAssessment> {
    let stripped = strip_code_fences(text);
    if let Some(assessment) = parse_assessment_candidate(&stripped, fallback_country_code)? {
        return Ok(assessment);
    }

    let Some(start) = stripped.find('{') else {
        anyhow::bail!("AI response did not contain a JSON object");
    };
    let Some(end) = stripped.rfind('}') else {
        anyhow::bail!("AI response contained an opening JSON brace but no closing brace");
    };
    let candidate = &stripped[start..=end];
    parse_assessment_candidate(candidate, fallback_country_code)?
        .ok_or_else(|| anyhow::anyhow!("AI response did not match assessment schema"))
}

fn parse_assessment_candidate(
    text: &str,
    fallback_country_code: Option<&str>,
) -> anyhow::Result<Option<AiAssessment>> {
    if let Ok(assessment) = serde_json::from_str::<AiAssessment>(text) {
        return Ok(Some(assessment));
    }

    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    parse_assessment_value(&value, fallback_country_code)
}

fn parse_assessment_value(
    value: &serde_json::Value,
    fallback_country_code: Option<&str>,
) -> anyhow::Result<Option<AiAssessment>> {
    if let Ok(assessment) = serde_json::from_value::<AiAssessment>(value.clone()) {
        return Ok(Some(assessment));
    }
    if let Some(assessment) = flexible_assessment_from_value(value, fallback_country_code) {
        return Ok(Some(assessment));
    }
    if let Some(object) = value.as_object() {
        for key in ["station", "assessment", "result", "output"] {
            if let Some(inner) = object.get(key)
                && let Some(assessment) = parse_assessment_value(inner, fallback_country_code)?
            {
                return Ok(Some(assessment));
            }
        }
        for inner in object.values() {
            if let Some(assessment) = parse_assessment_value(inner, fallback_country_code)? {
                return Ok(Some(assessment));
            }
        }
    }
    Ok(None)
}

fn flexible_assessment_from_value(
    value: &serde_json::Value,
    fallback_country_code: Option<&str>,
) -> Option<AiAssessment> {
    let accept = value
        .get("accept")
        .or_else(|| value.get("accepted"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let confidence = value
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.5) as f32;
    let canonical_name = value
        .get("canonical_name")
        .or_else(|| value.get("canonicalName"))
        .or_else(|| value.get("name"))
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let country_code = value
        .get("country_code")
        .or_else(|| value.get("countryCode"))
        .and_then(serde_json::Value::as_str)
        .or(fallback_country_code)
        .map(str::to_string)?;
    let tags = value
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let risks = value
        .get("risks")
        .and_then(serde_json::Value::as_array)
        .map(|risks| {
            risks
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Some(AiAssessment {
        accept,
        confidence,
        canonical_name,
        country_code,
        tags,
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        logo_url: value
            .get("logo_url")
            .or_else(|| value.get("logoUrl"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        risks,
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("No model reason provided.")
            .to_string(),
    })
}

fn response_preview(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(240).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_endpoint_appends_path_once() {
        assert_eq!(
            chat_endpoint("http://localhost:9000"),
            "http://localhost:9000/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://api.anthropic.com/v1/"),
            "https://api.anthropic.com/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("http://localhost:9000/v1/chat/completions"),
            "http://localhost:9000/v1/chat/completions"
        );
    }

    #[test]
    fn canonicalize_strips_slogans_and_noise() {
        assert_eq!(
            canonicalize_name("RADIO CENTRO: Calidad En Tu Vida"),
            "Radio Centro"
        );
        assert_eq!(canonicalize_name("Azul (89.5 FM) Livestream"), "Azul");
        assert_eq!(canonicalize_name("BBC RADIO LONDON"), "BBC Radio London");
        assert_eq!(canonicalize_name("Radio UNAM (FM)"), "Radio UNAM (FM)");
    }

    #[test]
    fn parses_wrapped_and_fenced_json() {
        let fenced = "```json\n{\"accept\": true, \"confidence\": 0.9, \"canonical_name\": \"X\", \"country_code\": \"MX\", \"reason\": \"ok\"}\n```";
        assert!(parse_assessment(fenced).is_ok());

        let wrapped = r#"{"assessment": {"accept": true, "confidence": 0.9, "canonical_name": "X", "country_code": "MX", "reason": "ok"}}"#;
        assert!(parse_assessment(wrapped).is_ok());

        let prose = "Here you go: {\"accept\": true, \"confidence\": 0.9, \"canonical_name\": \"X\", \"country_code\": \"MX\", \"reason\": \"ok\"} hope that helps";
        assert!(parse_assessment(prose).is_ok());
    }

    #[test]
    fn flexible_parse_fills_missing_fields() {
        let minimal = r#"{"name": "Radio X", "tags": ["pop"]}"#;
        let parsed = parse_assessment_with_fallback(minimal, Some("GB")).unwrap();
        assert_eq!(parsed.canonical_name, "Radio X");
        assert_eq!(parsed.country_code, "GB");
        assert!(parsed.accept);
    }

    #[test]
    fn validation_rejects_bad_output() {
        let assessment = AiAssessment {
            accept: true,
            confidence: 1.4,
            canonical_name: " ".to_string(),
            country_code: "gbr".to_string(),
            tags: vec![],
            description: None,
            logo_url: None,
            risks: vec![],
            reason: "r".to_string(),
        };
        let issues = validate(&assessment);
        assert!(issues.contains(&AiValidationIssue::EmptyName));
        assert!(issues.contains(&AiValidationIssue::InvalidCountryCode));
        assert!(issues.contains(&AiValidationIssue::InvalidConfidence));
    }
}
