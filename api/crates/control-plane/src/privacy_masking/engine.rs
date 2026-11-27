//! Linear-time regex compiler, deterministic HMAC-base32 placeholder generator, and payload masker.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use extension_contracts::{
    PrivacyMaskingLogMatchDto, PrivacyMaskingPreviewHitDto, PrivacyMaskingRuleKind,
    PrivacyMaskingRuleMode, PrivacyMaskingSubjectBindingDto, PrivacyMaskingSubjectKind,
    PrivacyMaskingWorkspacePolicyDto,
};
use hmac::{Hmac, KeyInit, Mac};
use regex::{Regex, RegexBuilder};
use serde_json::Value;
use sha2::Sha256;
use uuid::Uuid;

use super::builtins::{
    ceil_char_boundary, find_builtin_spec, floor_char_boundary, passes_boundary, passes_validator,
    BoundaryKind, BuiltinValidator,
};

type HmacSha256 = Hmac<Sha256>;

pub const MAX_CUSTOM_RULES: usize = 128;
pub const MAX_CUSTOM_PATTERN_LEN: usize = 512;
pub const MAX_ALLOWLIST_ITEMS: usize = 256;
pub const MAX_PLACEHOLDER_KIND_LEN: usize = 24;
const MAX_SNIPPET_CHARS: usize = 80;

#[derive(Debug, Clone, thiserror::Error)]
pub enum PrivacyPolicyCompileError {
    #[error("privacy_rule_limit_exceeded: custom rule count exceeds {MAX_CUSTOM_RULES}")]
    TooManyCustomRules,
    #[error("privacy_allowlist_limit_exceeded: allowlist exceeds {MAX_ALLOWLIST_ITEMS}")]
    TooManyAllowlistItems,
    #[error("privacy_rule_id_invalid: rule_id cannot be empty or duplicated: {0}")]
    InvalidRuleId(String),
    #[error("privacy_builtin_rule_unknown: unknown built-in rule code: {0}")]
    UnknownBuiltinRule(String),
    #[error("privacy_custom_regex_empty: custom rule `{0}` requires a non-empty regex pattern")]
    EmptyCustomRegex(String),
    #[error("privacy_custom_regex_too_long: custom rule `{0}` pattern exceeds {MAX_CUSTOM_PATTERN_LEN} chars")]
    CustomRegexTooLong(String),
    #[error("privacy_custom_regex_invalid: custom rule `{0}` failed to compile: {1}")]
    InvalidCustomRegex(String, String),
    #[error("privacy_custom_regex_matches_empty: custom rule `{0}` must not match empty string")]
    CustomRegexMatchesEmpty(String),
    #[error("privacy_subject_binding_invalid: binding `{0}` requires non-empty subject_id")]
    InvalidSubjectBinding(String),
}

impl PrivacyPolicyCompileError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooManyCustomRules => "privacy_rule_limit_exceeded",
            Self::TooManyAllowlistItems => "privacy_allowlist_limit_exceeded",
            Self::InvalidRuleId(_) => "privacy_rule_id_invalid",
            Self::UnknownBuiltinRule(_) => "privacy_builtin_rule_unknown",
            Self::EmptyCustomRegex(_) => "privacy_custom_regex_empty",
            Self::CustomRegexTooLong(_) => "privacy_custom_regex_too_long",
            Self::InvalidCustomRegex(_, _) => "privacy_custom_regex_invalid",
            Self::CustomRegexMatchesEmpty(_) => "privacy_custom_regex_matches_empty",
            Self::InvalidSubjectBinding(_) => "privacy_subject_binding_invalid",
        }
    }
}

pub fn sanitize_placeholder_kind(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().to_ascii_uppercase().chars() {
        if ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_' {
            out.push(ch);
        } else if matches!(ch, ' ' | '-' | '.') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    let mut normalized = if trimmed.is_empty() {
        "CUSTOM".to_string()
    } else if !trimmed.as_bytes()[0].is_ascii_uppercase() {
        format!("CUSTOM_{trimmed}")
    } else {
        trimmed
    };
    if normalized.len() > MAX_PLACEHOLDER_KIND_LEN {
        normalized.truncate(MAX_PLACEHOLDER_KIND_LEN);
        normalized = normalized.trim_end_matches('_').to_string();
    }
    if normalized.is_empty() {
        "CUSTOM".to_string()
    } else {
        normalized
    }
}

pub fn derive_hmac_subkey(master_key: &str) -> anyhow::Result<[u8; 32]> {
    if master_key.trim().is_empty() {
        anyhow::bail!("privacy_masking_master_key_missing");
    }
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(master_key.as_bytes())
        .map_err(|_| anyhow::anyhow!("privacy_masking_hmac_init_failed"))?;
    mac.update(b"1flowbase.privacy_masking.hmac_key.v1");
    let bytes = mac.finalize().into_bytes();
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

/// Deterministic 8-char RFC4648 lowercase base32 tag from HMAC-SHA256(subkey, workspace || kind || raw).
pub fn deterministic_placeholder(
    hmac_subkey: &[u8; 32],
    workspace_id: Uuid,
    placeholder_kind: &str,
    raw_value: &str,
) -> String {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(hmac_subkey)
        .expect("32-byte HMAC key is always valid");
    mac.update(workspace_id.as_bytes());
    mac.update(b"\x00");
    mac.update(placeholder_kind.as_bytes());
    mac.update(b"\x00");
    mac.update(raw_value.as_bytes());
    let digest = mac.finalize().into_bytes();
    let tag = encode_base32_lower_8(&digest[..5]);
    format!("{{{{{placeholder_kind}_{tag}}}}}")
}

fn encode_base32_lower_8(five_bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut bits: u64 = 0;
    for &byte in five_bytes.iter().take(5) {
        bits = (bits << 8) | u64::from(byte);
    }
    let mut out = String::with_capacity(8);
    for shift in (0..8).rev() {
        let idx = ((bits >> (shift * 5)) & 0x1f) as usize;
        out.push(ALPHABET[idx] as char);
    }
    out
}

pub fn placeholder_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\{\{[A-Z][A-Z0-9_]{0,23}_[a-z2-7]{8}\}\}")
            .expect("placeholder regex must compile")
    })
}

pub fn prefix_hint(raw: &str) -> String {
    let char_count = raw.chars().count();
    if char_count < 8 {
        return String::new();
    }
    let prefix_len = if char_count >= 16 { 4 } else { 2 };
    let head: String = raw.chars().take(prefix_len).collect();
    format!("{head}…")
}

#[derive(Clone)]
pub(crate) enum CompiledRuleMatcher {
    Builtin {
        builtin_code: String,
    },
    Custom {
        regex: Regex,
        capture_group: usize,
    },
}

#[derive(Clone)]
pub(crate) struct CompiledRule {
    pub rule_id: String,
    pub name: String,
    pub placeholder_kind: String,
    pub enabled: bool,
    pub priority: i32,
    pub matcher: CompiledRuleMatcher,
}

#[derive(Clone)]
pub struct CompiledWorkspacePolicy {
    pub installation_id: Option<Uuid>,
    pub policy_version: i64,
    pub enabled: bool,
    pub allowlist_lower: HashSet<String>,
    pub(crate) rules: Vec<CompiledRule>,
    pub subject_bindings: Vec<PrivacyMaskingSubjectBindingDto>,
}

impl CompiledWorkspacePolicy {
    pub fn compile(
        installation_id: Option<Uuid>,
        policy_version: i64,
        dto: &PrivacyMaskingWorkspacePolicyDto,
    ) -> Result<Self, PrivacyPolicyCompileError> {
        if dto.allowlist.len() > MAX_ALLOWLIST_ITEMS {
            return Err(PrivacyPolicyCompileError::TooManyAllowlistItems);
        }
        let mut seen_rule_ids = HashSet::new();
        let mut custom_count = 0usize;
        let mut compiled_rules = Vec::with_capacity(dto.rules.len());

        for rule in &dto.rules {
            let rule_id = rule.rule_id.trim();
            if rule_id.is_empty() || !seen_rule_ids.insert(rule_id.to_string()) {
                return Err(PrivacyPolicyCompileError::InvalidRuleId(
                    rule.rule_id.clone(),
                ));
            }
            let placeholder_kind = sanitize_placeholder_kind(&rule.placeholder_kind);
            let matcher = match rule.kind {
                PrivacyMaskingRuleKind::Builtin => {
                    let code = rule.builtin_code.as_deref().unwrap_or("").trim();
                    if find_builtin_spec(code).is_none() {
                        return Err(PrivacyPolicyCompileError::UnknownBuiltinRule(
                            code.to_string(),
                        ));
                    }
                    CompiledRuleMatcher::Builtin {
                        builtin_code: code.to_string(),
                    }
                }
                PrivacyMaskingRuleKind::CustomRegex => {
                    custom_count += 1;
                    if custom_count > MAX_CUSTOM_RULES {
                        return Err(PrivacyPolicyCompileError::TooManyCustomRules);
                    }
                    let pattern = rule.pattern.as_deref().unwrap_or("").trim();
                    if pattern.is_empty() {
                        return Err(PrivacyPolicyCompileError::EmptyCustomRegex(
                            rule_id.to_string(),
                        ));
                    }
                    if pattern.chars().count() > MAX_CUSTOM_PATTERN_LEN {
                        return Err(PrivacyPolicyCompileError::CustomRegexTooLong(
                            rule_id.to_string(),
                        ));
                    }
                    let regex = RegexBuilder::new(pattern)
                        .size_limit(1024 * 1024)
                        .dfa_size_limit(1024 * 1024)
                        .build()
                        .map_err(|err| {
                            PrivacyPolicyCompileError::InvalidCustomRegex(
                                rule_id.to_string(),
                                err.to_string(),
                            )
                        })?;
                    if regex.is_match("") {
                        return Err(PrivacyPolicyCompileError::CustomRegexMatchesEmpty(
                            rule_id.to_string(),
                        ));
                    }
                    let capture_group = if regex.captures_len() > 1 { 1 } else { 0 };
                    CompiledRuleMatcher::Custom {
                        regex,
                        capture_group,
                    }
                }
            };
            compiled_rules.push(CompiledRule {
                rule_id: rule_id.to_string(),
                name: rule.name.trim().to_string(),
                placeholder_kind,
                enabled: rule.enabled,
                priority: rule.priority,
                matcher,
            });
        }

        compiled_rules.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.rule_id.cmp(&b.rule_id)));

        let mut bindings = Vec::with_capacity(dto.subject_bindings.len());
        for binding in &dto.subject_bindings {
            if binding.subject_id.trim().is_empty() {
                return Err(PrivacyPolicyCompileError::InvalidSubjectBinding(
                    binding.binding_id.clone(),
                ));
            }
            bindings.push(binding.clone());
        }

        let allowlist_lower = dto
            .allowlist
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            installation_id,
            policy_version,
            enabled: dto.enabled,
            allowlist_lower,
            rules: compiled_rules,
            subject_bindings: bindings,
        })
    }

    pub fn enabled_rule_count(&self) -> u32 {
        self.rules.iter().filter(|r| r.enabled).count() as u32
    }

    pub fn active_subject_count(&self) -> u32 {
        self.subject_bindings
            .iter()
            .filter(|b| b.enabled)
            .count() as u32
    }

    pub fn matching_binding(
        &self,
        subject_kind: PrivacyMaskingSubjectKind,
        subject_id: &str,
    ) -> Option<&PrivacyMaskingSubjectBindingDto> {
        if !self.enabled {
            return None;
        }
        self.subject_bindings
            .iter()
            .find(|b| b.enabled && b.subject_kind == subject_kind && b.subject_id == subject_id)
    }

    pub(crate) fn rules_for_binding<'a>(
        &'a self,
        binding: &PrivacyMaskingSubjectBindingDto,
    ) -> Vec<&'a CompiledRule> {
        match binding.rule_mode {
            PrivacyMaskingRuleMode::AllEnabled => {
                self.rules.iter().filter(|rule| rule.enabled).collect()
            }
            PrivacyMaskingRuleMode::SelectedRules => {
                let selected: HashSet<&str> = binding.rule_ids.iter().map(String::as_str).collect();
                self.rules
                    .iter()
                    .filter(|rule| rule.enabled && selected.contains(rule.rule_id.as_str()))
                    .collect()
            }
        }
    }

    pub(crate) fn all_enabled_rules(&self) -> Vec<&CompiledRule> {
        self.rules.iter().filter(|rule| rule.enabled).collect()
    }
}

#[derive(Debug, Clone)]
pub struct StageMaskMatchAccumulator {
    pub rule_id: String,
    pub rule_name: String,
    pub placeholder_kind: String,
    pub placeholder: String,
    pub original_len: u32,
    pub prefix_hint: String,
    pub hit_count: u32,
    pub masked_snippet: String,
}

#[derive(Debug, Clone, Default)]
pub struct StageMaskState {
    /// placeholder -> original value (memory-only, never persisted)
    pub restore_map: HashMap<String, String>,
    /// Keyed by placeholder for deduplicated audit counters
    pub matches: BTreeMap<String, StageMaskMatchAccumulator>,
    pub total_hits: u32,
}

impl StageMaskState {
    pub fn to_log_matches(&self, restored_counts: &HashMap<String, u32>) -> Vec<PrivacyMaskingLogMatchDto> {
        self.matches
            .values()
            .map(|entry| PrivacyMaskingLogMatchDto {
                rule_id: entry.rule_id.clone(),
                rule_name: entry.rule_name.clone(),
                placeholder_kind: entry.placeholder_kind.clone(),
                placeholder: entry.placeholder.clone(),
                original_len: entry.original_len,
                prefix_hint: entry.prefix_hint.clone(),
                hit_count: entry.hit_count,
                restored_count: restored_counts
                    .get(&entry.placeholder)
                    .copied()
                    .unwrap_or(0),
                masked_snippet: entry.masked_snippet.clone(),
            })
            .collect()
    }

    pub fn to_preview_hits(&self) -> Vec<PrivacyMaskingPreviewHitDto> {
        self.matches
            .values()
            .map(|entry| PrivacyMaskingPreviewHitDto {
                rule_id: entry.rule_id.clone(),
                rule_name: entry.rule_name.clone(),
                placeholder_kind: entry.placeholder_kind.clone(),
                placeholder: entry.placeholder.clone(),
                original_len: entry.original_len,
                prefix_hint: entry.prefix_hint.clone(),
                hit_count: entry.hit_count,
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct CandidateSpan<'a> {
    start: usize,
    end: usize,
    rule: &'a CompiledRule,
}

/// Mask one string with an ordered list of compiled rules, recording deterministic placeholders.
pub(crate) fn mask_text_with_rules(
    text: &str,
    workspace_id: Uuid,
    hmac_subkey: &[u8; 32],
    allowlist_lower: &HashSet<String>,
    rules: &[&CompiledRule],
    state: &mut StageMaskState,
) -> String {
    if text.is_empty() || rules.is_empty() {
        return text.to_string();
    }

    // Existing placeholder spans are skipped by built-in rules to guarantee idempotency across turns/stages.
    let existing_placeholders: Vec<(usize, usize)> = if text.contains("{{") {
        placeholder_regex()
            .find_iter(text)
            .map(|m| (m.start(), m.end()))
            .collect()
    } else {
        Vec::new()
    };

    let mut candidates: Vec<CandidateSpan<'_>> = Vec::new();
    for &rule in rules {
        match &rule.matcher {
            CompiledRuleMatcher::Builtin { builtin_code } => {
                let Some(spec) = find_builtin_spec(builtin_code) else {
                    continue;
                };
                for pat in &spec.patterns {
                    collect_pattern_matches(
                        text,
                        &pat.regex,
                        pat.capture_group,
                        pat.boundary,
                        pat.validator,
                        true,
                        &existing_placeholders,
                        allowlist_lower,
                        rule,
                        &mut candidates,
                    );
                }
            }
            CompiledRuleMatcher::Custom {
                regex,
                capture_group,
            } => {
                collect_pattern_matches(
                    text,
                    regex,
                    *capture_group,
                    BoundaryKind::None,
                    BuiltinValidator::None,
                    false,
                    &existing_placeholders,
                    allowlist_lower,
                    rule,
                    &mut candidates,
                );
            }
        }
    }

    if candidates.is_empty() {
        return text.to_string();
    }

    // Deterministic non-overlapping selection: earliest start, then longest match, then rule priority.
    candidates.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| b.end.cmp(&a.end))
            .then_with(|| a.rule.priority.cmp(&b.rule.priority))
            .then_with(|| a.rule.rule_id.cmp(&b.rule.rule_id))
    });

    let mut chosen: Vec<CandidateSpan<'_>> = Vec::with_capacity(candidates.len());
    let mut cursor = 0usize;
    for candidate in candidates {
        if candidate.start >= cursor && candidate.end > candidate.start {
            cursor = candidate.end;
            chosen.push(candidate);
        }
    }

    if chosen.is_empty() {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len() + chosen.len() * 16);
    let mut last = 0usize;
    for span in &chosen {
        out.push_str(&text[last..span.start]);
        let raw = &text[span.start..span.end];
        let placeholder =
            deterministic_placeholder(hmac_subkey, workspace_id, &span.rule.placeholder_kind, raw);
        let placeholder_start_in_out = out.len();
        out.push_str(&placeholder);
        let placeholder_end_in_out = out.len();
        state
            .restore_map
            .entry(placeholder.clone())
            .or_insert_with(|| raw.to_string());
        state.total_hits = state.total_hits.saturating_add(1);

        let entry = state
            .matches
            .entry(placeholder.clone())
            .or_insert_with(|| StageMaskMatchAccumulator {
                rule_id: span.rule.rule_id.clone(),
                rule_name: if span.rule.name.is_empty() {
                    span.rule.rule_id.clone()
                } else {
                    span.rule.name.clone()
                },
                placeholder_kind: span.rule.placeholder_kind.clone(),
                placeholder: placeholder.clone(),
                original_len: raw.chars().count() as u32,
                prefix_hint: prefix_hint(raw),
                hit_count: 0,
                masked_snippet: String::new(),
            });
        entry.hit_count = entry.hit_count.saturating_add(1);
        if entry.masked_snippet.is_empty() {
            // Defer snippet extraction until full masked string is built so NO raw neighbor leaks.
            entry.masked_snippet = format!("{placeholder_start_in_out}:{placeholder_end_in_out}");
        }
        last = span.end;
    }
    out.push_str(&text[last..]);

    // Populate zero-plaintext snippets strictly from the fully masked output string `out`.
    for entry in state.matches.values_mut() {
        if let Some((start_s, end_s)) = entry.masked_snippet.split_once(':') {
            if let (Ok(s_idx), Ok(e_idx)) = (start_s.parse::<usize>(), end_s.parse::<usize>()) {
                if s_idx <= e_idx && e_idx <= out.len() {
                    let snip_start = floor_char_boundary(&out, s_idx.saturating_sub(24));
                    let snip_end = ceil_char_boundary(&out, (e_idx + 24).min(out.len()));
                    let mut snippet: String = out[snip_start..snip_end]
                        .chars()
                        .take(MAX_SNIPPET_CHARS)
                        .collect();
                    if snip_start > 0 {
                        snippet.insert(0, '…');
                    }
                    if snip_end < out.len() {
                        snippet.push('…');
                    }
                    entry.masked_snippet = snippet;
                }
            }
        }
    }

    out
}

#[allow(clippy::too_many_arguments)]
fn collect_pattern_matches<'a>(
    text: &str,
    regex: &Regex,
    capture_group: usize,
    boundary: BoundaryKind,
    validator: BuiltinValidator,
    skip_existing_placeholders: bool,
    existing_placeholders: &[(usize, usize)],
    allowlist_lower: &HashSet<String>,
    rule: &'a CompiledRule,
    out: &mut Vec<CandidateSpan<'a>>,
) {
    for captures in regex.captures_iter(text) {
        let Some(m) = captures.get(capture_group) else {
            continue;
        };
        let start = m.start();
        let end = m.end();
        if end <= start {
            continue;
        }
        if skip_existing_placeholders
            && existing_placeholders
                .iter()
                .any(|&(ps, pe)| start < pe && end > ps)
        {
            continue;
        }
        // Even custom rules must never re-mask an exact existing placeholder token itself.
        if existing_placeholders
            .iter()
            .any(|&(ps, pe)| start == ps && end == pe)
        {
            continue;
        }
        if !passes_boundary(text, start, end, boundary) {
            continue;
        }
        if !passes_validator(validator, text, start, end) {
            continue;
        }
        let raw = &text[start..end];
        if !allowlist_lower.is_empty() && allowlist_lower.contains(&raw.to_ascii_lowercase()) {
            continue;
        }
        out.push(CandidateSpan { start, end, rule });
    }
}

/// Recursively masks all string leaf values in a JSON tree, skipping structural metadata keys.
pub(crate) fn mask_json_value(
    value: &mut Value,
    workspace_id: Uuid,
    hmac_subkey: &[u8; 32],
    allowlist_lower: &HashSet<String>,
    rules: &[&CompiledRule],
    state: &mut StageMaskState,
) {
    match value {
        Value::String(text) => {
            let masked = mask_text_with_rules(
                text,
                workspace_id,
                hmac_subkey,
                allowlist_lower,
                rules,
                state,
            );
            *text = masked;
        }
        Value::Array(items) => {
            for item in items {
                mask_json_value(item, workspace_id, hmac_subkey, allowlist_lower, rules, state);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if is_non_content_wire_key(key) {
                    continue;
                }
                mask_json_value(item, workspace_id, hmac_subkey, allowlist_lower, rules, state);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_non_content_wire_key(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "role"
            | "id"
            | "call_id"
            | "tool_call_id"
            | "response_id"
            | "previous_response_id"
            | "model"
            | "mime_type"
            | "media_type"
            | "encrypted_content"
            | "signature"
    )
}
