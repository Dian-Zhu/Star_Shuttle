import { invoke } from '@tauri-apps/api/core';
import { get, writable } from 'svelte/store';

export type SkillMode = 'chat' | 'agent';
export type SkillAppliesTo = 'chat' | 'agent' | 'both';
export type AiSkillSourceType = 'builtin' | 'local_dir' | 'local_zip';
export type AiSkillTriggerMode = 'manual_only' | 'auto';

export interface AiSkillSummary {
  id: string;
  name: string;
  description: string;
  applies_to: SkillAppliesTo;
  allowed_tools: string[];
  recommended_sandbox: 'standard' | 'full' | null;
  starter_examples: string[];
  source_type: AiSkillSourceType;
  enabled: boolean;
  trusted: boolean;
  trigger_mode: AiSkillTriggerMode;
  content_hash: string | null;
}

export interface AiSkillCandidate {
  skill: AiSkillSummary;
  confidence: number;
  reason: string;
}

export interface AiSkillMatchResult {
  matched_skill_id: string | null;
  auto_applied: boolean;
  reason: string | null;
  alternatives: AiSkillCandidate[];
}

export const skillCatalog = writable<AiSkillSummary[]>([]);
export const installedSkillCatalog = writable<AiSkillSummary[]>([]);

let loadPromise: Promise<AiSkillSummary[]> | null = null;
let installedLoadPromise: Promise<AiSkillSummary[]> | null = null;

export async function loadSkillCatalog(
  force = false,
  mode?: SkillMode,
): Promise<AiSkillSummary[]> {
  const cached = get(skillCatalog);
  if (!force && cached.length > 0 && !mode) {
    return cached;
  }

  if (!force && loadPromise && !mode) {
    return loadPromise;
  }

  const request = invoke<AiSkillSummary[]>('ai_list_skills', {
    mode: mode ?? null,
  }).then((skills) => {
    if (!mode) {
      skillCatalog.set(skills);
    }
    return skills;
  });

  if (!mode) {
    loadPromise = request.finally(() => {
      loadPromise = null;
    });
    return loadPromise;
  }

  return request;
}

export async function loadInstalledSkills(
  force = false,
  mode?: SkillMode,
): Promise<AiSkillSummary[]> {
  const cached = get(installedSkillCatalog);
  if (!force && cached.length > 0 && !mode) {
    return cached;
  }

  if (!force && installedLoadPromise && !mode) {
    return installedLoadPromise;
  }

  const request = invoke<AiSkillSummary[]>('ai_list_installed_skills', {
    mode: mode ?? null,
  }).then((skills) => {
    if (!mode) {
      installedSkillCatalog.set(skills);
    }
    return skills;
  });

  if (!mode) {
    installedLoadPromise = request.finally(() => {
      installedLoadPromise = null;
    });
    return installedLoadPromise;
  }

  return request;
}

export async function installSkillFromDir(path: string): Promise<AiSkillSummary> {
  const skill = await invoke<AiSkillSummary>('ai_install_skill_from_dir', { path });
  await loadInstalledSkills(true);
  await loadSkillCatalog(true);
  return skill;
}

export async function setSkillEnabled(
  skillId: string,
  enabled: boolean,
): Promise<AiSkillSummary> {
  const skill = await invoke<AiSkillSummary>('ai_set_skill_enabled', { skillId, enabled });
  patchSkill(skill);
  return skill;
}

export async function setSkillTrusted(
  skillId: string,
  trusted: boolean,
): Promise<AiSkillSummary> {
  const skill = await invoke<AiSkillSummary>('ai_set_skill_trusted', { skillId, trusted });
  patchSkill(skill);
  return skill;
}

export async function removeSkill(skillId: string): Promise<void> {
  await invoke('ai_remove_skill', { skillId });
  installedSkillCatalog.update((skills) => skills.filter((skill) => skill.id !== skillId));
  skillCatalog.update((skills) => skills.filter((skill) => skill.id !== skillId));
}

export async function reloadSkills(mode?: SkillMode): Promise<AiSkillSummary[]> {
  const skills = await invoke<AiSkillSummary[]>('ai_reload_skills', { mode: mode ?? null });
  if (!mode) {
    installedSkillCatalog.set(skills);
    skillCatalog.set(skills.filter(isRuntimeAvailableSkill));
  }
  return skills;
}

export async function matchSkills(
  input: string,
  mode: SkillMode,
): Promise<AiSkillMatchResult> {
  const trimmed = input.trim();
  if (!trimmed) {
    return emptyMatch();
  }

  try {
    return await invoke<AiSkillMatchResult>('ai_match_skills', { input: trimmed, mode });
  } catch {
    const localSkills = filterSkillsByMode(get(skillCatalog), mode);
    const fallbackCandidates = localSkills
      .filter((skill) => {
        const haystacks = [skill.id, skill.name, skill.description, ...skill.starter_examples];
        return haystacks.some((value) => value.toLowerCase().includes(trimmed.toLowerCase()));
      })
      .slice(0, 3)
      .map((skill, index) => ({
        skill,
        confidence: Math.max(100 - index * 10, 40),
        reason: '本地回退匹配',
      }));

    const top = fallbackCandidates[0];
    return {
      matched_skill_id: top?.skill.id ?? null,
      auto_applied: !!top,
      reason: top?.reason ?? null,
      alternatives: fallbackCandidates,
    };
  }
}

export function filterSkillsByMode(skills: AiSkillSummary[], mode: SkillMode): AiSkillSummary[] {
  return skills.filter((skill) => skill.applies_to === 'both' || skill.applies_to === mode);
}

export function filterInstalledSkillsByMode(
  skills: AiSkillSummary[],
  mode: SkillMode,
): AiSkillSummary[] {
  return filterSkillsByMode(skills, mode);
}

export function isRuntimeAvailableSkill(skill: AiSkillSummary): boolean {
  return skill.enabled && skill.trusted;
}

export function getSkillLabel(skillId: string | null | undefined): string | null {
  if (!skillId) {
    return null;
  }
  return (
    get(skillCatalog).find((skill) => skill.id === skillId)?.name ??
    get(installedSkillCatalog).find((skill) => skill.id === skillId)?.name ??
    skillId
  );
}

export function getSkillById(skillId: string | null | undefined): AiSkillSummary | null {
  if (!skillId) {
    return null;
  }
  return (
    get(skillCatalog).find((skill) => skill.id === skillId) ??
    get(installedSkillCatalog).find((skill) => skill.id === skillId) ??
    null
  );
}

function patchSkill(nextSkill: AiSkillSummary) {
  installedSkillCatalog.update((skills) => upsertSkill(skills, nextSkill));
  skillCatalog.update((skills) => {
    const next = upsertSkill(skills, nextSkill);
    return next.filter(isRuntimeAvailableSkill);
  });
}

function upsertSkill(skills: AiSkillSummary[], nextSkill: AiSkillSummary): AiSkillSummary[] {
  const next = [...skills];
  const index = next.findIndex((skill) => skill.id === nextSkill.id);
  if (index >= 0) {
    next[index] = nextSkill;
  } else {
    next.push(nextSkill);
  }
  return next.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'));
}

function emptyMatch(): AiSkillMatchResult {
  return {
    matched_skill_id: null,
    auto_applied: false,
    reason: null,
    alternatives: [],
  };
}
