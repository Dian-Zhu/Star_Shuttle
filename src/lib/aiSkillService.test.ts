import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import {
  filterSkillsByMode,
  getSkillById,
  getSkillLabel,
  installedSkillCatalog,
  isRuntimeAvailableSkill,
  skillCatalog,
  type AiSkillSummary,
} from './aiSkillService';

const SKILLS: AiSkillSummary[] = [
  {
    id: 'log_diagnostics',
    name: '日志诊断',
    description: 'logs',
    applies_to: 'both',
    allowed_tools: ['execute_command'],
    recommended_sandbox: 'standard',
    starter_examples: [],
    source_type: 'builtin',
    enabled: true,
    trusted: true,
    trigger_mode: 'auto',
    content_hash: null,
  },
  {
    id: 'agent_only',
    name: 'Agent Only',
    description: 'agent',
    applies_to: 'agent',
    allowed_tools: ['get_system_info'],
    recommended_sandbox: null,
    starter_examples: [],
    source_type: 'local_dir',
    enabled: false,
    trusted: false,
    trigger_mode: 'manual_only',
    content_hash: 'abc',
  },
];

describe('aiSkillService', () => {
  beforeEach(() => {
    skillCatalog.set(SKILLS.filter((skill) => skill.enabled && skill.trusted));
    installedSkillCatalog.set(SKILLS);
  });

  it('filters skills by mode', () => {
    expect(filterSkillsByMode(get(installedSkillCatalog), 'chat').map((skill) => skill.id)).toEqual(
      ['log_diagnostics'],
    );
    expect(filterSkillsByMode(get(installedSkillCatalog), 'agent').map((skill) => skill.id)).toEqual(
      ['log_diagnostics', 'agent_only'],
    );
  });

  it('resolves skill labels from both runtime and installed catalogs', () => {
    expect(getSkillLabel('log_diagnostics')).toBe('日志诊断');
    expect(getSkillById('agent_only')?.allowed_tools).toEqual(['get_system_info']);
    expect(getSkillLabel(null)).toBeNull();
  });

  it('recognizes runtime-available skills', () => {
    expect(isRuntimeAvailableSkill(SKILLS[0])).toBe(true);
    expect(isRuntimeAvailableSkill(SKILLS[1])).toBe(false);
  });
});
