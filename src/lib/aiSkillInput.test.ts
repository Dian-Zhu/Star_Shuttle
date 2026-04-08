import { describe, expect, it } from 'vitest';
import type { AiSkillSummary } from './aiSkillService';
import {
  buildSkillOptions,
  findSkillCommand,
  removeSkillCommand,
} from './aiSkillInput';

const SKILLS: AiSkillSummary[] = [
  {
    id: 'log_diagnostics',
    name: '日志诊断',
    description: '分析日志中的错误和异常模式。',
    applies_to: 'both',
    allowed_tools: [],
    recommended_sandbox: 'standard',
    starter_examples: ['分析日志错误'],
    source_type: 'builtin',
    enabled: true,
    trusted: true,
    trigger_mode: 'auto',
    content_hash: null,
  },
  {
    id: 'docker_troubleshooting',
    name: 'Docker 排障',
    description: '定位容器启动失败和网络问题。',
    applies_to: 'agent',
    allowed_tools: [],
    recommended_sandbox: 'full',
    starter_examples: ['检查 docker 容器状态'],
    source_type: 'builtin',
    enabled: true,
    trusted: true,
    trigger_mode: 'auto',
    content_hash: null,
  },
];

describe('aiSkillInput', () => {
  it('finds a slash skill command at the cursor', () => {
    expect(findSkillCommand('请帮我 /doc', '请帮我 /doc'.length)).toEqual({
      start: 4,
      end: 8,
      query: 'doc',
    });
  });

  it('ignores filesystem paths and nested slash tokens', () => {
    expect(findSkillCommand('查看 /var/log/nginx/error.log', '查看 /var'.length)).toBeNull();
    expect(findSkillCommand('/tmp/file', '/tmp/file'.length)).toBeNull();
    expect(findSkillCommand('请帮我 /docker 排查容器启动失败', '请帮我 /docker 排查容器启动失败'.length)).toBeNull();
  });

  it('builds skill options with a default clear option', () => {
    const options = buildSkillOptions(SKILLS, '');

    expect(options.map((option) => option.id)).toEqual([
      null,
      'log_diagnostics',
      'docker_troubleshooting',
    ]);
  });

  it('filters options by query across ids and names', () => {
    expect(buildSkillOptions(SKILLS, 'docker').map((option) => option.id)).toEqual([
      'docker_troubleshooting',
    ]);
    expect(buildSkillOptions(SKILLS, '日志').map((option) => option.id)).toEqual([
      'log_diagnostics',
    ]);
  });

  it('removes the slash command and keeps surrounding content tidy', () => {
    const input = '请帮我 /docker 排查容器启动失败';
    const match = findSkillCommand(input, '请帮我 /docker'.length);
    expect(match).not.toBeNull();

    expect(removeSkillCommand(input, match!)).toEqual({
      text: '请帮我 排查容器启动失败',
      cursor: 4,
    });
  });
});
