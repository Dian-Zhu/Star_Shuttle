import type { AiSkillSummary } from './aiSkillService';

export interface SkillCommandMatch {
  start: number;
  end: number;
  query: string;
}

export interface SkillOption {
  id: string | null;
  name: string;
  description: string;
  recommendedSandbox: 'standard' | 'full' | null;
}

export function findSkillCommand(text: string, cursor: number): SkillCommandMatch | null {
  const safeCursor = Math.max(0, Math.min(cursor, text.length));
  const beforeCursor = text.slice(0, safeCursor);
  const slashIndex = beforeCursor.lastIndexOf('/');

  if (slashIndex === -1) {
    return null;
  }

  if (slashIndex > 0 && !/\s/.test(text[slashIndex - 1] ?? '')) {
    return null;
  }

  const tokenBeforeCursor = text.slice(slashIndex + 1, safeCursor);
  if (/\s/.test(tokenBeforeCursor) || !isSkillQuerySegment(tokenBeforeCursor)) {
    return null;
  }

  const afterCursor = text.slice(safeCursor);
  const nextWhitespace = afterCursor.search(/\s/);
  const tokenAfterCursor = nextWhitespace === -1 ? afterCursor : afterCursor.slice(0, nextWhitespace);

  if (!isSkillQuerySegment(tokenAfterCursor)) {
    return null;
  }

  return {
    start: slashIndex,
    end: safeCursor + tokenAfterCursor.length,
    query: `${tokenBeforeCursor}${tokenAfterCursor}`.trim().toLowerCase(),
  };
}

export function buildSkillOptions(skills: AiSkillSummary[], query: string): SkillOption[] {
  const normalizedQuery = query.trim().toLowerCase();
  const defaultOption: SkillOption = {
    id: null,
    name: '不使用 Skill',
    description: '发送原始请求，不附加额外的 Skill 约束。',
    recommendedSandbox: null,
  };

  const skillOptions = skills
    .filter((skill) => matchesSkillQuery(skill, normalizedQuery))
    .map<SkillOption>((skill) => ({
      id: skill.id,
      name: skill.name,
      description: skill.description,
      recommendedSandbox: skill.recommended_sandbox,
    }));

  if (!normalizedQuery) {
    return [defaultOption, ...skillOptions];
  }

  if (matchesOptionQuery(defaultOption, normalizedQuery)) {
    return [defaultOption, ...skillOptions];
  }

  return skillOptions;
}

export function removeSkillCommand(
  text: string,
  match: SkillCommandMatch,
): { text: string; cursor: number } {
  const before = text.slice(0, match.start);
  const after = text.slice(match.end);
  const trimmedBefore = before.replace(/[ \t]+$/, '');
  const trimmedAfter = after.replace(/^[ \t]+/, '');
  const needsSpace =
    trimmedBefore.length > 0 &&
    trimmedAfter.length > 0 &&
    !trimmedBefore.endsWith('\n') &&
    !trimmedAfter.startsWith('\n');

  const nextText = `${trimmedBefore}${needsSpace ? ' ' : ''}${trimmedAfter}`;

  return {
    text: nextText,
    cursor: trimmedBefore.length + (needsSpace ? 1 : 0),
  };
}

function isSkillQuerySegment(segment: string): boolean {
  return !segment.includes('/');
}

function matchesSkillQuery(skill: AiSkillSummary, query: string): boolean {
  if (!query) {
    return true;
  }

  const haystacks = [
    skill.id,
    skill.name,
    skill.description,
    ...skill.starter_examples,
  ].map((value) => value.toLowerCase());

  return haystacks.some((value) => value.includes(query));
}

function matchesOptionQuery(option: SkillOption, query: string): boolean {
  return [option.name, option.description]
    .map((value) => value.toLowerCase())
    .some((value) => value.includes(query));
}
