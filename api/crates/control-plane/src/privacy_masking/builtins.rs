//! Built-in privacy redaction rule catalog and anti-false-positive validators.
use extension_contracts::PrivacyMaskingBuiltinRuleDescriptorDto;
use regex::{Regex, RegexBuilder};
use std::sync::OnceLock;

pub const BUILTIN_PRIVATE_KEY: &str = "builtin.private_key";
pub const BUILTIN_API_KEY_PREFIXED: &str = "builtin.api_key_prefixed";
pub const BUILTIN_JWT_TOKEN: &str = "builtin.jwt_token";
pub const BUILTIN_URL_PASSWORD: &str = "builtin.url_password";
pub const BUILTIN_CONFIG_SECRET_ASSIGNMENT: &str = "builtin.config_secret_assignment";
pub const BUILTIN_CJK_NAMED_SECRET: &str = "builtin.cjk_named_secret";
pub const BUILTIN_EMAIL: &str = "builtin.email";
pub const BUILTIN_CN_PHONE: &str = "builtin.cn_phone";
pub const BUILTIN_CN_ID_CARD: &str = "builtin.cn_id_card";
pub const BUILTIN_BANK_CARD: &str = "builtin.bank_card";
pub const BUILTIN_CRYPTO_PRIVATE_KEY: &str = "builtin.crypto_private_key";
pub const BUILTIN_IPV4_PUBLIC: &str = "builtin.ipv4_public";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinValidator {
    None,
    NotASlug,
    SecretValue,
    UrlPassword,
    ConfigSecret,
    CjkNamedSecret,
    RealEmail,
    CnPhoneContext,
    CnIdCardChecksum,
    BankCardLuhn,
    CryptoKeyContext,
    PublicIpv4Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryKind {
    None,
    TokenChars,
    Alnum,
    EmailLocal,
}

pub(crate) struct CompiledBuiltinPattern {
    pub regex: Regex,
    pub capture_group: usize,
    pub boundary: BoundaryKind,
    pub validator: BuiltinValidator,
}

pub(crate) struct BuiltinRuleSpec {
    pub descriptor: PrivacyMaskingBuiltinRuleDescriptorDto,
    pub patterns: Vec<CompiledBuiltinPattern>,
}

fn compile_linear(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(2 * 1024 * 1024)
        .build()
        .expect("built-in privacy masking regex must compile")
}

pub(crate) fn builtin_specs() -> &'static [BuiltinRuleSpec] {
    static SPECS: OnceLock<Vec<BuiltinRuleSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        vec![
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_PRIVATE_KEY.into(),
                    category: "secrets".into(),
                    placeholder_kind: "PRIVATE_KEY".into(),
                    default_enabled: true,
                    display_name_zh: "PEM / SSH 私钥块".into(),
                    display_name_en: "PEM / SSH Private Keys".into(),
                    description_zh: "匹配完整 PEM、OpenSSH、PGP 私钥文本块。".into(),
                    description_en: "Matches complete PEM, OpenSSH, and PGP private key blocks."
                        .into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(
                        r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----[\s\S]+?-----END (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----",
                    ),
                    capture_group: 0,
                    boundary: BoundaryKind::None,
                    validator: BuiltinValidator::None,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_API_KEY_PREFIXED.into(),
                    category: "secrets".into(),
                    placeholder_kind: "API_KEY".into(),
                    default_enabled: true,
                    display_name_zh: "主流云与 AI 厂商 API Key".into(),
                    display_name_en: "Prefixed API Keys".into(),
                    description_zh: "匹配 OpenAI/Anthropic(sk-)、火山方舟(ark-)、GitHub/GitLab、Google(AIza)、AWS(AKIA/ASIA)、Stripe、Slack、Cloudflare、HuggingFace 等带特征前缀的密钥。".into(),
                    description_en: "Matches prefixed keys from OpenAI/Anthropic (sk-), Volcengine Ark (ark-), GitHub/GitLab, Google (AIza), AWS (AKIA/ASIA), Stripe, Slack, Cloudflare, HuggingFace, and relays.".into(),
                },
                patterns: vec![
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r"sk-(?:ant-|proj-|or-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}",
                        ),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::NotASlug,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(r"[a-z]{2,4}_sk_[A-Za-z0-9_-]{20,}"),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::SecretValue,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(r"ark-[A-Za-z0-9][A-Za-z0-9_-]{15,}"),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::SecretValue,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r"(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|glpat-[A-Za-z0-9_-]{20,}|AIza[0-9A-Za-z_-]{35}|xox[abposr]-[0-9A-Za-z-]{10,}|(?:sk|rk)_live_[0-9A-Za-z]{20,}|hf_[A-Za-z0-9]{30,}|gsk_[A-Za-z0-9]{40,}|xai-[A-Za-z0-9]{40,}|npm_[A-Za-z0-9]{36}|pypi-[A-Za-z0-9_-]{50,}|dop_v1_[a-f0-9]{64}|SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}|cf(?:ut|at|k|oat)_[A-Za-z0-9_-]{40,}|v1\.0-[A-Za-z0-9]{16,}-[A-Za-z0-9]{40,})",
                        ),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::None,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(r"(?:AKIA|ASIA)[0-9A-Z]{16}"),
                        capture_group: 0,
                        boundary: BoundaryKind::Alnum,
                        validator: BuiltinValidator::None,
                    },
                ],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_JWT_TOKEN.into(),
                    category: "secrets".into(),
                    placeholder_kind: "TOKEN".into(),
                    default_enabled: true,
                    display_name_zh: "JWT 与通道令牌".into(),
                    display_name_en: "JWT & Tunnel Tokens".into(),
                    description_zh: "匹配三段式 JWT 令牌（eyJ...）与 Cloudflare 通道 Token。".into(),
                    description_en: "Matches three-part JWT tokens (eyJ...) and Cloudflare tunnel tokens.".into(),
                },
                patterns: vec![
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r"eyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
                        ),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::None,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(r"eyJhIjoi[A-Za-z0-9+/_-]{40,}={0,2}"),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::None,
                    },
                ],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_URL_PASSWORD.into(),
                    category: "secrets".into(),
                    placeholder_kind: "PASSWORD".into(),
                    default_enabled: true,
                    display_name_zh: "连接串与 URL 密码".into(),
                    display_name_en: "Connection String / URL Passwords".into(),
                    description_zh: "仅替换数据库连接串或 URL 中 user:password@host 的密码段，保留协议与主机。".into(),
                    description_en: "Replaces only the password segment in scheme://user:password@host URLs while keeping scheme and host intact.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(
                        r#"[A-Za-z][A-Za-z0-9+.-]*://[^\s:@/?#"'<>]*:([^\s@/?#"'<>]{3,})@"#,
                    ),
                    capture_group: 1,
                    boundary: BoundaryKind::None,
                    validator: BuiltinValidator::UrlPassword,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_CONFIG_SECRET_ASSIGNMENT.into(),
                    category: "secrets".into(),
                    placeholder_kind: "SECRET".into(),
                    default_enabled: true,
                    display_name_zh: "配置密钥赋值与认证请求头".into(),
                    display_name_en: "Config Secret Assignments & Auth Headers".into(),
                    description_zh: "匹配 .env/JSON/YAML 中的 password、secret、token、api_key 赋值及 Authorization/Cookie/X-Auth-Key 头，自动跳过占位变量与版本号。".into(),
                    description_en: "Matches password/secret/token/api_key assignments and Authorization/Cookie/X-Auth-Key headers while skipping template vars and versions.".into(),
                },
                patterns: vec![
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?i)[A-Za-z0-9_.-]*(?:password|passwd|passphrase|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|priv[_-]?key|credential)[A-Za-z0-9_.-]*(?:\\?["'])?[ \t]*[:=][ \t]*(?:\\?["'])?([A-Za-z0-9_\-./+=~!@#%^&*]{8,})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::None,
                        validator: BuiltinValidator::ConfigSecret,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?:^|[^A-Za-z0-9_.-])[A-Za-z0-9_.-]*_(?:KEY|PASS)(?:\\?["'])?[ \t]*[:=][ \t]*(?:\\?["'])?([A-Za-z0-9_\-./+=~!@#%^&*]{8,})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::None,
                        validator: BuiltinValidator::ConfigSecret,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?im)^[ \t]*(?:authorization|cookie|x-auth-(?:user-service-)?key)[ \t]*:[ \t]*(\S[^\r\n]{7,})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::None,
                        validator: BuiltinValidator::ConfigSecret,
                    },
                ],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_CJK_NAMED_SECRET.into(),
                    category: "secrets".into(),
                    placeholder_kind: "SECRET".into(),
                    default_enabled: true,
                    display_name_zh: "中文对话中的密码与密钥陈述".into(),
                    display_name_en: "CJK Natural-Language Secrets".into(),
                    description_zh: "匹配中文语句中的「密码是…」「密钥：…」「令牌为…」等敏感值，自动排除文件路径与位数描述。".into(),
                    description_en: "Matches secrets introduced in CJK prose (e.g. 密码是..., 密钥：...) while ignoring file paths and bit-lengths.".into(),
                },
                patterns: vec![
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?:密码|密碼|口令|PIN码|PIN碼|安全码|安全碼)[ \t]*(?:[:：=][ \t]*|[为為是][ \t]*[:：]?[ \t]*)?(?:["'“「])?([A-Za-z0-9_\-./+=~!@#%^&*]{4,64})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::None,
                        validator: BuiltinValidator::CjkNamedSecret,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?:密钥|密鑰|秘钥|私钥|私鑰|金钥|令牌|授权码|授權碼)[ \t]*(?:[:：=][ \t]*|[为為是][ \t]*[:：]?[ \t]*)?(?:["'“「])?([A-Za-z0-9_\-./+=~!@#%^&*]{6,64})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::None,
                        validator: BuiltinValidator::CjkNamedSecret,
                    },
                ],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_EMAIL.into(),
                    category: "personal".into(),
                    placeholder_kind: "EMAIL".into(),
                    default_enabled: true,
                    display_name_zh: "电子邮箱地址".into(),
                    display_name_en: "Email Addresses".into(),
                    description_zh: "匹配真实邮箱地址，自动放行 example.com / .test / .invalid / .localhost 示例域名。".into(),
                    description_en: "Matches real email addresses while exempting example.com, .test, .invalid, and .localhost.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(
                        r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}",
                    ),
                    capture_group: 0,
                    boundary: BoundaryKind::EmailLocal,
                    validator: BuiltinValidator::RealEmail,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_CN_PHONE.into(),
                    category: "personal".into(),
                    placeholder_kind: "PHONE".into(),
                    default_enabled: true,
                    display_name_zh: "中国大陆手机号（防订单号误杀）".into(),
                    display_name_en: "Mainland China Mobile Numbers".into(),
                    description_zh: "匹配 11 位大陆手机号，结合上下文字段名自动规避 order_id、订单号、流水号等长整型编号误杀。".into(),
                    description_en: "Matches 11-digit mainland China mobile numbers with context guards against order_id/uid/serial false positives.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(r"(?:\+?86[- ]?)?1[3-9]\d{9}"),
                    capture_group: 0,
                    boundary: BoundaryKind::Alnum,
                    validator: BuiltinValidator::CnPhoneContext,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_CN_ID_CARD.into(),
                    category: "personal".into(),
                    placeholder_kind: "ID_CARD".into(),
                    default_enabled: true,
                    display_name_zh: "中国大陆身份证号（含校验位）".into(),
                    display_name_en: "Mainland China Resident ID".into(),
                    description_zh: "匹配 18 位身份证号并执行 GB 11643 模 11 校验位与合法日期校验。".into(),
                    description_en: "Matches 18-digit China Resident IDs with GB 11643 modulo-11 checksum and calendar validation.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(
                        r"[1-9]\d{5}(?:18|19|20)\d{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12]\d|3[01])\d{3}[\dXx]",
                    ),
                    capture_group: 0,
                    boundary: BoundaryKind::Alnum,
                    validator: BuiltinValidator::CnIdCardChecksum,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_BANK_CARD.into(),
                    category: "personal".into(),
                    placeholder_kind: "BANK_CARD".into(),
                    default_enabled: true,
                    display_name_zh: "银行卡号（Luhn 校验）".into(),
                    display_name_en: "Bank / Payment Card Numbers (Luhn)".into(),
                    description_zh: "匹配 13–19 位银行卡/信用卡号并通过 Luhn 算法校验，避免普通数字串误判。".into(),
                    description_en: "Matches 13-19 digit card numbers verified with the Luhn checksum.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(r"[3-6]\d{3}(?:[ -]?\d{4}){2,3}(?:[ -]?\d{1,3})?"),
                    capture_group: 0,
                    boundary: BoundaryKind::Alnum,
                    validator: BuiltinValidator::BankCardLuhn,
                }],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_CRYPTO_PRIVATE_KEY.into(),
                    category: "crypto".into(),
                    placeholder_kind: "CRYPTO_KEY".into(),
                    default_enabled: true,
                    display_name_zh: "加密钱包私钥".into(),
                    display_name_en: "Crypto Wallet Private Keys".into(),
                    description_zh: "匹配带前缀钱包私钥（suiprivkey1、nsec1、AGE-SECRET-KEY-1、xprv）及私钥上下文中的 64 位 EVM 十六进制私钥，不误杀交易哈希。".into(),
                    description_en: "Matches prefixed wallet private keys and contextual 64-hex EVM private keys without masking bare tx hashes.".into(),
                },
                patterns: vec![
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r"(?:suiprivkey1[0-9a-z]{50,}|nsec1[0-9a-z]{50,}|AGE-SECRET-KEY-1[0-9A-Z]{50,}|ed25519-priv-0x[0-9a-fA-F]{64}|xprv[1-9A-HJ-NP-Za-km-z]{90,})",
                        ),
                        capture_group: 0,
                        boundary: BoundaryKind::TokenChars,
                        validator: BuiltinValidator::None,
                    },
                    CompiledBuiltinPattern {
                        regex: compile_linear(
                            r#"(?i)(?:private[_-]?key|priv[_-]?key|wallet[_-]?key|signer|deployer|私钥|私鑰)[ \t"']*[:=]?[ \t"']*(0x[0-9a-fA-F]{64}|[0-9a-fA-F]{64})"#,
                        ),
                        capture_group: 1,
                        boundary: BoundaryKind::Alnum,
                        validator: BuiltinValidator::CryptoKeyContext,
                    },
                ],
            },
            BuiltinRuleSpec {
                descriptor: PrivacyMaskingBuiltinRuleDescriptorDto {
                    builtin_code: BUILTIN_IPV4_PUBLIC.into(),
                    category: "personal".into(),
                    placeholder_kind: "IP_ADDRESS".into(),
                    default_enabled: false,
                    display_name_zh: "公网 IPv4 地址（默认关闭）".into(),
                    display_name_en: "Public IPv4 Addresses (Off by Default)".into(),
                    description_zh: "匹配非私网/非回环公网 IPv4 地址，排除 10/127/172.16-31/192.168 段及版本号文本。".into(),
                    description_en: "Matches non-private IPv4 addresses while excluding RFC1918/loopback ranges and version strings.".into(),
                },
                patterns: vec![CompiledBuiltinPattern {
                    regex: compile_linear(
                        r"(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}",
                    ),
                    capture_group: 0,
                    boundary: BoundaryKind::Alnum,
                    validator: BuiltinValidator::PublicIpv4Context,
                }],
            },
        ]
    })
}

pub fn builtin_rule_descriptors() -> Vec<PrivacyMaskingBuiltinRuleDescriptorDto> {
    builtin_specs()
        .iter()
        .map(|spec| spec.descriptor.clone())
        .collect()
}

pub(crate) fn find_builtin_spec(code: &str) -> Option<&'static BuiltinRuleSpec> {
    builtin_specs()
        .iter()
        .find(|spec| spec.descriptor.builtin_code == code)
}

pub(crate) fn passes_boundary(text: &str, start: usize, end: usize, kind: BoundaryKind) -> bool {
    let forbidden = match kind {
        BoundaryKind::None => return true,
        BoundaryKind::TokenChars => is_token_char,
        BoundaryKind::Alnum => is_ascii_alnum,
        BoundaryKind::EmailLocal => is_email_local_char,
    };
    if let Some(prev) = text[..start].chars().next_back() {
        if forbidden(prev) {
            return false;
        }
    }
    if let Some(next) = text[end..].chars().next() {
        if forbidden(next) {
            return false;
        }
    }
    true
}

pub(crate) fn passes_validator(
    validator: BuiltinValidator,
    text: &str,
    start: usize,
    end: usize,
) -> bool {
    let value = &text[start..end];
    match validator {
        BuiltinValidator::None => true,
        BuiltinValidator::NotASlug => not_a_slug(value),
        BuiltinValidator::SecretValue => is_secret_value(value),
        BuiltinValidator::UrlPassword => not_a_variable(value),
        BuiltinValidator::ConfigSecret => is_config_secret(value),
        BuiltinValidator::CjkNamedSecret => is_cjk_secret(text, start, end),
        BuiltinValidator::RealEmail => is_real_email(value),
        BuiltinValidator::CnPhoneContext => is_cn_phone_context(text, start, end),
        BuiltinValidator::CnIdCardChecksum => is_valid_cn_id_card(value),
        BuiltinValidator::BankCardLuhn => is_valid_luhn_card(value),
        BuiltinValidator::CryptoKeyContext => not_a_variable(value),
        BuiltinValidator::PublicIpv4Context => is_public_ipv4_context(text, start, end),
    }
}

fn is_ascii_alnum(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn is_email_local_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '%' | '+' | '-')
}

fn not_a_variable(value: &str) -> bool {
    let Some(first) = value.chars().next() else {
        return false;
    };
    if matches!(first, '$' | '%' | '{' | '<' | '[' | '*') {
        return false;
    }
    let trimmed = value.trim_matches(|c| matches!(c, '*' | 'x' | 'X' | '.'));
    !trimmed.is_empty()
}

fn not_a_slug(value: &str) -> bool {
    if !not_a_variable(value) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("sk-test-") || lower.contains("example") || lower.contains("placeholder") {
        return false;
    }
    value.chars().any(|c| c.is_ascii_digit()) || value.chars().any(|c| c.is_ascii_uppercase())
}

fn looks_like_path_or_version(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if value.starts_with("~/")
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with('/')
        || value.contains("/etc/")
        || value.contains("/home/")
        || value.contains("/usr/")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".toml")
    {
        return true;
    }
    // Semver / ISO timestamp guard (e.g. 0.15.0-rc1, 2026-08-30T11:44:31)
    let bytes = value.as_bytes();
    if bytes.len() >= 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
    {
        return true;
    }
    value
        .strip_prefix('v')
        .unwrap_or(value)
        .split('.')
        .take(3)
        .filter(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        .count()
        >= 3
}

fn is_secret_value(value: &str) -> bool {
    if !not_a_variable(value) || looks_like_path_or_version(value) {
        return false;
    }
    let has_letter = value.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    has_letter && (has_digit || value.len() >= 16)
}

fn is_config_secret(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 8 || !not_a_variable(trimmed) || looks_like_path_or_version(trimmed) {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "null" | "none" | "undefined" | "password" | "secret" | "token" | "changeme"
    ) {
        return false;
    }
    let has_letter = trimmed.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    has_letter && (has_digit || trimmed.len() >= 16 || trimmed.contains(' '))
}

fn is_cjk_secret(text: &str, start: usize, end: usize) -> bool {
    let value = &text[start..end];
    if !not_a_variable(value) || looks_like_path_or_version(value) {
        return false;
    }
    // Reject pure numeric quantities followed by unit nouns (e.g. "密钥为 2048 位")
    if value.chars().all(|c| c.is_ascii_digit()) {
        let after = text[end..].trim_start();
        if after.starts_with('位')
            || after.starts_with("字节")
            || after.starts_with("毫秒")
            || after.starts_with("秒")
            || after.starts_with("次")
        {
            return false;
        }
        return value.len() >= 6;
    }
    true
}

fn is_real_email(value: &str) -> bool {
    let Some((_, domain)) = value.rsplit_once('@') else {
        return false;
    };
    let lower = domain.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "example.com" | "example.org" | "example.net" | "localhost" | "test.local"
    ) || lower.ends_with(".example")
        || lower.ends_with(".test")
        || lower.ends_with(".invalid")
        || lower.ends_with(".localhost")
    {
        return false;
    }
    true
}

fn is_cn_phone_context(text: &str, start: usize, end: usize) -> bool {
    let value = &text[start..end];
    // Explicit +86 / 86 prefix is always a phone number
    if value.len() > 11 {
        return true;
    }
    let window_start = floor_char_boundary(text, start.saturating_sub(48));
    let before = text[window_start..start].trim_end();
    let lower_before = before.to_ascii_lowercase();

    // If preceded by assignment operator (`:`, `=`, `>`), inspect field name
    if let Some(sep_idx) = lower_before.rfind(|c| matches!(c, ':' | '=' | '>')) {
        let after_sep = lower_before[sep_idx + 1..].trim();
        let clean_after = after_sep.trim_matches(|c| matches!(c, '"' | '\'' | ' ' | '\t'));
        if clean_after.is_empty() {
            let field_slice = lower_before[..sep_idx]
                .trim_end_matches(|c: char| matches!(c, '"' | '\'' | ' ' | '\t' | '/'));
            let field_name = field_slice
                .rsplit(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .next()
                .unwrap_or("");
            if !field_name.is_empty() {
                const PHONE_WORDS: &[&str] = &[
                    "phone",
                    "mobile",
                    "tel",
                    "telephone",
                    "cell",
                    "contact",
                    "whatsapp",
                    "sms",
                    "msisdn",
                    "fax",
                    "手机",
                    "电话",
                    "電話",
                    "联系",
                    "聯繫",
                    "联络",
                ];
                return PHONE_WORDS.iter().any(|word| field_name.contains(word));
            }
        }
    }

    // In prose, reject numbers immediately preceded by order/id/ticket keywords
    const ID_WORDS: &[&str] = &[
        "order", "order_id", "uid", "id", "no.", "no", "#", "number", "ticket", "invoice",
        "订单号", "订单编号", "订单", "单号", "编号", "流水号", "工单号", "工单", "运单号",
        "快递单号", "交易号", "序号", "账单号",
    ];
    let prose_tail = lower_before
        .trim_end_matches(|c: char| matches!(c, ':' | '：' | '=' | '"' | '\'' | ' ' | '\t'));
    if ID_WORDS.iter().any(|word| prose_tail.ends_with(word)) {
        return false;
    }
    true
}

fn is_valid_cn_id_card(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 18 {
        return false;
    }
    let year: u32 = match value[6..10].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let month: u32 = match value[10..12].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let day: u32 = match value[12..14].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    if day == 0 || day > max_day {
        return false;
    }
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CHECK_CODES: [u8; 11] = *b"10X98765432";
    let mut sum = 0_u32;
    for i in 0..17 {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
        sum += u32::from(bytes[i] - b'0') * WEIGHTS[i];
    }
    let expected = CHECK_CODES[(sum % 11) as usize];
    bytes[17].to_ascii_uppercase() == expected
}

fn is_valid_luhn_card(value: &str) -> bool {
    let digits: Vec<u32> = value
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| u32::from(b - b'0'))
        .collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    if digits.iter().all(|&d| d == digits[0]) {
        return false;
    }
    let mut sum = 0_u32;
    let mut double = false;
    for &digit in digits.iter().rev() {
        let mut n = digit;
        if double {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        double = !double;
    }
    sum % 10 == 0
}

fn is_public_ipv4_context(text: &str, start: usize, end: usize) -> bool {
    let value = &text[start..end];
    let octets: Vec<u8> = value
        .split('.')
        .filter_map(|p| p.parse::<u8>().ok())
        .collect();
    if octets.len() != 4 {
        return false;
    }
    let [a, b, _, _] = [octets[0], octets[1], octets[2], octets[3]];
    // Exclude private, loopback, link-local, unspecified, CGNAT, multicast
    if a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || a >= 224
    {
        return false;
    }
    let window_start = floor_char_boundary(text, start.saturating_sub(24));
    let before = text[window_start..start].to_ascii_lowercase();
    if before.contains("version") || before.ends_with('v') {
        return false;
    }
    true
}

pub(crate) fn floor_char_boundary(text: &str, mut idx: usize) -> usize {
    idx = idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}
