use crate::modules::ai::sandbox::SandboxMode;
use crate::modules::db::{
    ai_store::{
        delete_skill_record, get_skill_record, list_skill_records, save_skill_record,
        set_skill_enabled as set_skill_enabled_record,
        set_skill_trusted as set_skill_trusted_record, StoredSkillRecord,
    },
    DatabaseManager,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const LINUX_EMERGENCY_RESPONSE_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/ai_skills/linux_emergency_response/SKILL.md"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMode {
    Chat,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSkillAppliesTo {
    Chat,
    Agent,
    Both,
}

impl AiSkillAppliesTo {
    fn matches_mode(&self, mode: SkillMode) -> bool {
        matches!(
            (self, mode),
            (Self::Both, _) | (Self::Chat, SkillMode::Chat) | (Self::Agent, SkillMode::Agent)
        )
    }

    fn from_stored(value: &str) -> Result<Self, String> {
        match value {
            "chat" => Ok(Self::Chat),
            "agent" => Ok(Self::Agent),
            "both" => Ok(Self::Both),
            other => Err(format!("未知 applies_to: {}", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSkillSourceType {
    Builtin,
    LocalDir,
    LocalZip,
}

impl AiSkillSourceType {
    fn from_stored(value: &str) -> Result<Self, String> {
        match value {
            "builtin" => Ok(Self::Builtin),
            "local_dir" => Ok(Self::LocalDir),
            "local_zip" => Ok(Self::LocalZip),
            other => Err(format!("未知 skill source_type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSkillTriggerMode {
    ManualOnly,
    Auto,
}

impl AiSkillTriggerMode {
    fn from_stored(value: &str) -> Result<Self, String> {
        match value {
            "manual_only" => Ok(Self::ManualOnly),
            "auto" => Ok(Self::Auto),
            other => Err(format!("未知 trigger_mode: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub applies_to: AiSkillAppliesTo,
    pub allowed_tools: Vec<String>,
    pub recommended_sandbox: Option<SandboxMode>,
    pub starter_examples: Vec<String>,
    pub source_type: AiSkillSourceType,
    pub enabled: bool,
    pub trusted: bool,
    pub trigger_mode: AiSkillTriggerMode,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AiSkill {
    pub summary: AiSkillSummary,
    pub system_prompt_fragment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSkillCandidate {
    pub skill: AiSkillSummary,
    pub confidence: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSkillMatchResult {
    pub matched_skill_id: Option<String>,
    pub auto_applied: bool,
    pub reason: Option<String>,
    pub alternatives: Vec<AiSkillCandidate>,
}

#[derive(Debug, Clone)]
struct SkillDefinition {
    summary: AiSkillSummary,
    system_prompt_fragment: String,
    trigger_regex: Option<String>,
    match_keywords: Vec<String>,
}

#[derive(Debug, Default)]
struct ParsedSkillManifest {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    trigger_regex: Option<String>,
}

fn builtin_skills() -> Vec<SkillDefinition> {
    vec![
        SkillDefinition {
            summary: AiSkillSummary {
                id: "linux_emergency_response".to_string(),
                name: "Linux 应急响应".to_string(),
                description: "面向 Linux 入侵排查、后门检查和可疑痕迹分析的应急响应技能。"
                    .to_string(),
                applies_to: AiSkillAppliesTo::Both,
                allowed_tools: vec![
                    "execute_command".to_string(),
                    "get_system_info".to_string(),
                ],
                recommended_sandbox: Some(SandboxMode::Standard),
                starter_examples: vec![
                    "做一次 Linux 主机应急响应排查".to_string(),
                    "检查这台服务器有没有入侵和后门痕迹".to_string(),
                    "分析异常登录、计划任务和可疑外联".to_string(),
                ],
                source_type: AiSkillSourceType::Builtin,
                enabled: true,
                trusted: true,
                trigger_mode: AiSkillTriggerMode::Auto,
                content_hash: None,
            },
            system_prompt_fragment: format!(
                "Skill: Linux emergency response.\n\
Focus on structured incident response for Linux hosts.\n\
Prioritize evidence collection, suspicious account/process/network/file analysis, persistence checks, and risk grading.\n\
Prefer read-only inspection commands first. After each command, summarize facts, suspicious findings, normal findings, and next steps.\n\
When evidence is incomplete, state uncertainty explicitly.\n\n\
Packaged skill guide:\n{}",
                LINUX_EMERGENCY_RESPONSE_SKILL_MD
            ),
            trigger_regex: Some("(?i)(应急响应|入侵|后门|webshell|异常登录|可疑外联)".to_string()),
            match_keywords: vec![
                "应急响应".to_string(),
                "入侵".to_string(),
                "后门".to_string(),
                "webshell".to_string(),
                "异常登录".to_string(),
                "可疑外联".to_string(),
            ],
        },
        SkillDefinition {
            summary: AiSkillSummary {
                id: "log_diagnostics".to_string(),
                name: "日志诊断".to_string(),
                description: "聚焦日志、报错、堆栈和系统事件分析。".to_string(),
                applies_to: AiSkillAppliesTo::Both,
                allowed_tools: vec![
                    "execute_command".to_string(),
                    "get_system_info".to_string(),
                ],
                recommended_sandbox: Some(SandboxMode::Standard),
                starter_examples: vec![
                    "分析 nginx 错误日志".to_string(),
                    "定位最近的服务异常".to_string(),
                ],
                source_type: AiSkillSourceType::Builtin,
                enabled: true,
                trusted: true,
                trigger_mode: AiSkillTriggerMode::Auto,
                content_hash: None,
            },
            system_prompt_fragment: "Skill: Log diagnostics.\n\
Prioritize logs, recent failures, process state, service state, and correlated symptoms.\n\
When summarizing, separate observed facts from hypotheses."
                .to_string(),
            trigger_regex: Some("(?i)(日志|报错|异常|stack|trace|error)".to_string()),
            match_keywords: vec![
                "日志".to_string(),
                "报错".to_string(),
                "异常".to_string(),
                "error".to_string(),
                "stack".to_string(),
            ],
        },
        SkillDefinition {
            summary: AiSkillSummary {
                id: "system_health_check".to_string(),
                name: "系统巡检".to_string(),
                description: "用于快速检查主机整体健康度和资源状态。".to_string(),
                applies_to: AiSkillAppliesTo::Both,
                allowed_tools: vec!["get_system_info".to_string()],
                recommended_sandbox: Some(SandboxMode::Standard),
                starter_examples: vec![
                    "做一次主机健康检查".to_string(),
                    "看看 CPU、内存和磁盘有没有异常".to_string(),
                ],
                source_type: AiSkillSourceType::Builtin,
                enabled: true,
                trusted: true,
                trigger_mode: AiSkillTriggerMode::Auto,
                content_hash: None,
            },
            system_prompt_fragment: "Skill: System health check.\n\
Focus on OS, CPU, memory, disk, and broad system health indicators.\n\
Avoid speculative remediation unless the collected signals support it."
                .to_string(),
            trigger_regex: Some("(?i)(巡检|健康检查|cpu|内存|磁盘|load|uptime)".to_string()),
            match_keywords: vec![
                "巡检".to_string(),
                "健康检查".to_string(),
                "cpu".to_string(),
                "内存".to_string(),
                "磁盘".to_string(),
            ],
        },
        SkillDefinition {
            summary: AiSkillSummary {
                id: "docker_troubleshooting".to_string(),
                name: "Docker 排障".to_string(),
                description: "用于分析容器运行状态、日志和宿主机资源影响。".to_string(),
                applies_to: AiSkillAppliesTo::Both,
                allowed_tools: vec![
                    "execute_command".to_string(),
                    "get_system_info".to_string(),
                ],
                recommended_sandbox: Some(SandboxMode::Standard),
                starter_examples: vec![
                    "检查正在运行的容器和异常日志".to_string(),
                    "看看 docker 服务为什么不稳定".to_string(),
                ],
                source_type: AiSkillSourceType::Builtin,
                enabled: true,
                trusted: true,
                trigger_mode: AiSkillTriggerMode::Auto,
                content_hash: None,
            },
            system_prompt_fragment: "Skill: Docker troubleshooting.\n\
Focus on container status, container logs, daemon state, image/runtime issues, and host resource pressure.\n\
Prefer read-only inspection commands first."
                .to_string(),
            trigger_regex: Some("(?i)(docker|容器|container|镜像|daemon)".to_string()),
            match_keywords: vec![
                "docker".to_string(),
                "容器".to_string(),
                "container".to_string(),
                "镜像".to_string(),
            ],
        },
    ]
}

pub fn list_skills(
    db: &Arc<Mutex<DatabaseManager>>,
    mode: Option<SkillMode>,
) -> Result<Vec<AiSkillSummary>, String> {
    let mut skills: Vec<AiSkillSummary> = builtin_skills()
        .into_iter()
        .filter(|skill| {
            mode.map(|value| skill.summary.applies_to.matches_mode(value))
                .unwrap_or(true)
        })
        .map(|skill| skill.summary)
        .collect();

    skills.extend(
        load_external_skills(db, mode, false)?
            .into_iter()
            .map(|skill| skill.summary),
    );

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

pub fn list_installed_skills(
    db: &Arc<Mutex<DatabaseManager>>,
    mode: Option<SkillMode>,
) -> Result<Vec<AiSkillSummary>, String> {
    let mut skills: Vec<AiSkillSummary> = builtin_skills()
        .into_iter()
        .filter(|skill| {
            mode.map(|value| skill.summary.applies_to.matches_mode(value))
                .unwrap_or(true)
        })
        .map(|skill| skill.summary)
        .collect();

    skills.extend(
        load_external_skills(db, mode, true)?
            .into_iter()
            .map(|skill| skill.summary),
    );

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

pub fn resolve_skill(
    db: &Arc<Mutex<DatabaseManager>>,
    skill_id: Option<&str>,
    mode: SkillMode,
) -> Result<Option<AiSkill>, String> {
    let Some(skill_id) = skill_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if let Some(skill) = builtin_skills()
        .into_iter()
        .find(|item| item.summary.id == skill_id)
    {
        if !skill.summary.applies_to.matches_mode(mode) {
            return Err(format!("skill {} 不支持当前模式 {:?}", skill_id, mode));
        }
        return Ok(Some(AiSkill {
            summary: skill.summary,
            system_prompt_fragment: skill.system_prompt_fragment,
        }));
    }

    let records = load_external_skills(db, Some(mode), false)?;
    let skill = records
        .into_iter()
        .find(|item| item.summary.id == skill_id)
        .ok_or_else(|| format!("未知 skill: {}", skill_id))?;

    Ok(Some(AiSkill {
        summary: skill.summary,
        system_prompt_fragment: skill.system_prompt_fragment,
    }))
}

pub fn match_skills(
    db: &Arc<Mutex<DatabaseManager>>,
    input: &str,
    mode: SkillMode,
) -> Result<AiSkillMatchResult, String> {
    let query = input.trim();
    if query.is_empty() {
        return Ok(AiSkillMatchResult {
            matched_skill_id: None,
            auto_applied: false,
            reason: None,
            alternatives: Vec::new(),
        });
    }

    let mut candidates = Vec::new();
    for skill in builtin_skills()
        .into_iter()
        .filter(|skill| skill.summary.applies_to.matches_mode(mode))
        .chain(load_external_skills(db, Some(mode), false)?.into_iter())
    {
        if let Some((confidence, reason)) = score_skill_match(query, &skill)? {
            candidates.push(AiSkillCandidate {
                skill: skill.summary,
                confidence,
                reason,
            });
        }
    }

    candidates.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| a.skill.name.cmp(&b.skill.name))
    });
    candidates.truncate(3);

    let top = candidates.first().cloned();
    Ok(AiSkillMatchResult {
        matched_skill_id: top.as_ref().map(|item| item.skill.id.clone()),
        auto_applied: top
            .as_ref()
            .map(|item| item.confidence >= 80)
            .unwrap_or(false),
        reason: top.as_ref().map(|item| item.reason.clone()),
        alternatives: candidates,
    })
}

pub fn install_skill_from_dir(
    db: &Arc<Mutex<DatabaseManager>>,
    path: &Path,
) -> Result<AiSkillSummary, String> {
    let source_dir = path.canonicalize().map_err(|e| e.to_string())?;
    let manifest_path = source_dir.join("SKILL.md");
    if !manifest_path.exists() {
        return Err("目录中缺少 SKILL.md".to_string());
    }

    let manifest_text = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let parsed = parse_skill_manifest(&manifest_text);
    let skill_id = parsed
        .id
        .as_deref()
        .map(sanitize_skill_id)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            sanitize_skill_id(
                source_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("imported_skill"),
            )
        });
    let name = parsed.name.unwrap_or_else(|| skill_id.clone());
    let description = parsed
        .description
        .unwrap_or_else(|| "用户导入的本地 Skill".to_string());

    let install_root = installed_skill_root()?;
    fs::create_dir_all(&install_root).map_err(|e| e.to_string())?;
    let installed_dir = install_root.join(&skill_id);
    if installed_dir.exists() {
        return Err(format!("skill {} 已存在，请先删除后再导入", skill_id));
    }

    copy_dir_recursive(&source_dir, &installed_dir)?;

    let installed_manifest = installed_dir.join("SKILL.md");
    let content_hash = calculate_content_hash(&manifest_text);
    let record = StoredSkillRecord {
        id: skill_id.clone(),
        name: name.clone(),
        description: description.clone(),
        applies_to: "both".to_string(),
        source_type: "local_dir".to_string(),
        source_path: installed_dir.to_string_lossy().to_string(),
        manifest_path: installed_manifest.to_string_lossy().to_string(),
        trigger_mode: "auto".to_string(),
        trigger_regex: parsed.trigger_regex,
        match_keywords_json: serde_json::to_string(&derive_match_keywords(
            &skill_id,
            &name,
            &description,
            &[],
        ))
        .map_err(|e| e.to_string())?,
        allowed_tools_json: serde_json::to_string(&vec![
            "execute_command".to_string(),
            "get_system_info".to_string(),
        ])
        .map_err(|e| e.to_string())?,
        starter_examples_json: "[]".to_string(),
        recommended_sandbox: Some("standard".to_string()),
        enabled: false,
        trusted: false,
        content_hash: Some(content_hash.clone()),
        installed_at: String::new(),
        updated_at: String::new(),
    };

    let db = db.lock().map_err(|e| e.to_string())?;
    save_skill_record(db.conn(), &record).map_err(|e| e.to_string())?;
    Ok(summary_from_record(&record)?)
}

pub fn set_skill_enabled(
    db: &Arc<Mutex<DatabaseManager>>,
    skill_id: &str,
    enabled: bool,
) -> Result<AiSkillSummary, String> {
    ensure_custom_skill(skill_id)?;
    let db = db.lock().map_err(|e| e.to_string())?;
    set_skill_enabled_record(db.conn(), skill_id, enabled).map_err(|e| e.to_string())?;
    let record = get_skill_record(db.conn(), skill_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("skill {} 不存在", skill_id))?;
    summary_from_record(&record)
}

pub fn set_skill_trusted(
    db: &Arc<Mutex<DatabaseManager>>,
    skill_id: &str,
    trusted: bool,
) -> Result<AiSkillSummary, String> {
    ensure_custom_skill(skill_id)?;
    let db = db.lock().map_err(|e| e.to_string())?;
    set_skill_trusted_record(db.conn(), skill_id, trusted).map_err(|e| e.to_string())?;
    let record = get_skill_record(db.conn(), skill_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("skill {} 不存在", skill_id))?;
    summary_from_record(&record)
}

pub fn remove_skill(db: &Arc<Mutex<DatabaseManager>>, skill_id: &str) -> Result<(), String> {
    ensure_custom_skill(skill_id)?;
    let record = {
        let db = db.lock().map_err(|e| e.to_string())?;
        let record = get_skill_record(db.conn(), skill_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("skill {} 不存在", skill_id))?;
        delete_skill_record(db.conn(), skill_id).map_err(|e| e.to_string())?;
        record
    };

    let path = PathBuf::from(record.source_path);
    if path.exists() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn reload_skills(
    db: &Arc<Mutex<DatabaseManager>>,
    mode: Option<SkillMode>,
) -> Result<Vec<AiSkillSummary>, String> {
    let stale_ids = {
        let db = db.lock().map_err(|e| e.to_string())?;
        list_skill_records(db.conn())
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|record| !Path::new(&record.manifest_path).exists())
            .map(|record| record.id)
            .collect::<Vec<_>>()
    };

    if !stale_ids.is_empty() {
        let db = db.lock().map_err(|e| e.to_string())?;
        for skill_id in stale_ids {
            delete_skill_record(db.conn(), &skill_id).map_err(|e| e.to_string())?;
        }
    }

    list_installed_skills(db, mode)
}

fn load_external_skills(
    db: &Arc<Mutex<DatabaseManager>>,
    mode: Option<SkillMode>,
    include_inactive: bool,
) -> Result<Vec<SkillDefinition>, String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    let records = list_skill_records(db.conn()).map_err(|e| e.to_string())?;
    records
        .into_iter()
        .map(record_to_skill_definition)
        .filter_map(|result| match result {
            Ok(skill) => Some(Ok(skill)),
            Err(error) => Some(Err(error)),
        })
        .filter(|result| {
            result
                .as_ref()
                .map(|skill| {
                    mode.map(|value| skill.summary.applies_to.matches_mode(value))
                        .unwrap_or(true)
                        && (include_inactive || (skill.summary.enabled && skill.summary.trusted))
                })
                .unwrap_or(true)
        })
        .collect()
}

fn record_to_skill_definition(record: StoredSkillRecord) -> Result<SkillDefinition, String> {
    let summary = summary_from_record(&record)?;
    let manifest_text = fs::read_to_string(&record.manifest_path).map_err(|e| e.to_string())?;
    let keywords = serde_json::from_str::<Vec<String>>(&record.match_keywords_json)
        .map_err(|e| e.to_string())?;

    Ok(SkillDefinition {
        system_prompt_fragment: format!(
            "Installed Skill: {}\nSource: {:?}\n\n{}",
            summary.name, summary.source_type, manifest_text
        ),
        trigger_regex: record.trigger_regex.clone(),
        match_keywords: keywords,
        summary,
    })
}

fn summary_from_record(record: &StoredSkillRecord) -> Result<AiSkillSummary, String> {
    Ok(AiSkillSummary {
        id: record.id.clone(),
        name: record.name.clone(),
        description: record.description.clone(),
        applies_to: AiSkillAppliesTo::from_stored(&record.applies_to)?,
        allowed_tools: serde_json::from_str(&record.allowed_tools_json)
            .map_err(|e| e.to_string())?,
        recommended_sandbox: match record.recommended_sandbox.as_deref() {
            Some("full") => Some(SandboxMode::Full),
            Some("standard") => Some(SandboxMode::Standard),
            Some(other) => return Err(format!("未知 recommended_sandbox: {}", other)),
            None => None,
        },
        starter_examples: serde_json::from_str(&record.starter_examples_json)
            .map_err(|e| e.to_string())?,
        source_type: AiSkillSourceType::from_stored(&record.source_type)?,
        enabled: record.enabled,
        trusted: record.trusted,
        trigger_mode: AiSkillTriggerMode::from_stored(&record.trigger_mode)?,
        content_hash: record.content_hash.clone(),
    })
}

fn score_skill_match(
    input: &str,
    skill: &SkillDefinition,
) -> Result<Option<(u32, String)>, String> {
    let mut score = 0u32;
    let mut reasons = Vec::new();
    let query = input.to_lowercase();

    if let Some(pattern) = skill.trigger_regex.as_deref() {
        let regex = Regex::new(pattern).map_err(|e| e.to_string())?;
        if regex.is_match(input) {
            score += 200;
            reasons.push("触发规则命中".to_string());
        }
    }

    let skill_id = skill.summary.id.to_lowercase();
    if query.contains(&skill_id) {
        score += 120;
        reasons.push("命中 skill id".to_string());
    }

    let skill_name = skill.summary.name.to_lowercase();
    if query.contains(&skill_name) {
        score += 100;
        reasons.push("命中 skill 名称".to_string());
    }

    let description = skill.summary.description.to_lowercase();
    if !description.is_empty() && query.contains(&description) {
        score += 60;
        reasons.push("命中 skill 描述".to_string());
    }

    if skill
        .summary
        .starter_examples
        .iter()
        .any(|item| !item.is_empty() && query.contains(&item.to_lowercase()))
    {
        score += 50;
        reasons.push("命中示例".to_string());
    }

    if skill
        .match_keywords
        .iter()
        .any(|item| item.len() > 1 && query.contains(&item.to_lowercase()))
    {
        score += 40;
        reasons.push("命中关键词".to_string());
    }

    if score == 0 {
        return Ok(None);
    }

    Ok(Some((score, reasons.join("，"))))
}

fn parse_skill_manifest(contents: &str) -> ParsedSkillManifest {
    if contents.trim_start().starts_with("---") {
        return parse_frontmatter_manifest(contents);
    }

    let mut parsed = ParsedSkillManifest::default();
    for line in contents.lines().take(40) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        apply_manifest_key_value(&mut parsed, key.trim(), value.trim());
    }
    parsed
}

fn parse_frontmatter_manifest(contents: &str) -> ParsedSkillManifest {
    let mut parsed = ParsedSkillManifest::default();
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some("---") {
        return parsed;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        apply_manifest_key_value(&mut parsed, key.trim(), value.trim());
    }
    parsed
}

fn apply_manifest_key_value(parsed: &mut ParsedSkillManifest, key: &str, value: &str) {
    let normalized = value.trim_matches('"').trim_matches('\'').to_string();
    match key {
        "id" => parsed.id = Some(normalized),
        "name" => parsed.name = Some(normalized),
        "description" => parsed.description = Some(normalized),
        "trigger_regex" => parsed.trigger_regex = Some(normalized),
        _ => {}
    }
}

fn derive_match_keywords(
    skill_id: &str,
    name: &str,
    description: &str,
    starter_examples: &[String],
) -> Vec<String> {
    let mut values = Vec::new();
    values.push(skill_id.to_string());
    values.push(name.to_string());
    values.push(description.to_string());
    values.extend(starter_examples.iter().cloned());
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
    values
}

fn sanitize_skill_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    let trimmed = sanitized.trim_matches('_').to_string();
    while trimmed.contains("__") {
        return trimmed.replace("__", "_");
    }
    trimmed
}

fn calculate_content_hash(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn installed_skill_root() -> Result<PathBuf, String> {
    let mut root = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定本地 skill 存储目录".to_string())?;
    root.push("star_shuttle");
    root.push("ai_skills");
    Ok(root)
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();
        let target_path = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            fs::copy(&entry_path, &target_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn ensure_custom_skill(skill_id: &str) -> Result<(), String> {
    if builtin_skills()
        .iter()
        .any(|skill| skill.summary.id == skill_id)
    {
        return Err("内置 skill 不支持此操作".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        install_skill_from_dir, list_installed_skills, list_skills, match_skills, reload_skills,
        resolve_skill, set_skill_enabled, set_skill_trusted, SkillMode,
    };
    use crate::modules::db::DatabaseManager;
    use std::fs;
    use std::sync::{Arc, Mutex};

    fn in_memory_db() -> Arc<Mutex<DatabaseManager>> {
        Arc::new(Mutex::new(
            DatabaseManager::new(":memory:").expect("in-memory db"),
        ))
    }

    #[test]
    fn filters_agent_skills_by_mode() {
        let db = in_memory_db();
        let skills = list_skills(&db, Some(SkillMode::Agent)).expect("list");
        assert!(!skills.is_empty());
        assert!(skills.iter().all(|skill| {
            matches!(
                skill.applies_to,
                super::AiSkillAppliesTo::Agent | super::AiSkillAppliesTo::Both
            )
        }));
    }

    #[test]
    fn lists_linux_emergency_response_skill() {
        let db = in_memory_db();
        let skills = list_skills(&db, None).expect("list");
        let skill = skills
            .into_iter()
            .find(|item| item.id == "linux_emergency_response")
            .expect("linux emergency response skill should exist");
        assert_eq!(skill.name, "Linux 应急响应");
        assert_eq!(
            skill.allowed_tools,
            vec!["execute_command".to_string(), "get_system_info".to_string()]
        );
    }

    #[test]
    fn resolves_known_skill() {
        let db = in_memory_db();
        let skill = resolve_skill(&db, Some("system_health_check"), SkillMode::Agent)
            .expect("resolve")
            .expect("skill");
        assert_eq!(skill.summary.allowed_tools, vec!["get_system_info"]);
    }

    #[test]
    fn resolves_linux_emergency_response_skill() {
        let db = in_memory_db();
        let skill = resolve_skill(&db, Some("linux_emergency_response"), SkillMode::Agent)
            .expect("resolve")
            .expect("skill");
        assert!(skill
            .system_prompt_fragment
            .contains("Linux emergency response"));
        assert!(skill.system_prompt_fragment.contains("hostname"));
    }

    #[test]
    fn rejects_unknown_skill() {
        let db = in_memory_db();
        let err = resolve_skill(&db, Some("missing"), SkillMode::Chat).expect_err("should fail");
        assert!(err.contains("未知 skill"));
    }

    #[test]
    fn matches_builtin_skill_by_security_keywords() {
        let db = in_memory_db();
        let result =
            match_skills(&db, "帮我做 Linux 入侵和后门排查", SkillMode::Agent).expect("match");
        assert_eq!(
            result.matched_skill_id.as_deref(),
            Some("linux_emergency_response")
        );
        assert!(result.auto_applied);
    }

    #[test]
    fn installs_and_enables_external_skill() {
        let db = in_memory_db();
        let temp_dir =
            std::env::temp_dir().join(format!("star-shuttle-skill-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        fs::write(
            temp_dir.join("SKILL.md"),
            "# SKILL.md\nid: test_ir\nname: Test IR\ndescription: test skill\ntrigger_regex: (?i)test ir\n",
        )
        .expect("write skill");

        let installed = install_skill_from_dir(&db, &temp_dir).expect("install");
        assert_eq!(installed.id, "test_ir");
        assert!(!installed.enabled);
        assert!(!installed.trusted);

        let enabled = set_skill_enabled(&db, "test_ir", true).expect("enable");
        assert!(enabled.enabled);

        let trusted = set_skill_trusted(&db, "test_ir", true).expect("trust");
        assert!(trusted.trusted);

        let listed = list_installed_skills(&db, None).expect("list installed");
        assert!(listed.iter().any(|skill| skill.id == "test_ir"));

        let reloaded = reload_skills(&db, None).expect("reload");
        assert!(reloaded.iter().any(|skill| skill.id == "test_ir"));

        let _ = fs::remove_dir_all(temp_dir);
    }
}
