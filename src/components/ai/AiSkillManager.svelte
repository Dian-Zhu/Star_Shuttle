<script lang="ts">
  import { confirm, open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import {
    installedSkillCatalog,
    installSkillFromDir,
    loadInstalledSkills,
    removeSkill,
    setSkillEnabled,
    setSkillTrusted,
  } from '../../lib/aiSkillService';

  let loading = false;
  let busySkillId = '';
  let message = '';
  let error = '';
  let collapsed = true;

  onMount(() => {
    void refresh();
  });

  async function refresh() {
    loading = true;
    error = '';
    try {
      await loadInstalledSkills(true);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  async function handleImportDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择 Skill 目录',
    });
    if (typeof selected !== 'string' || !selected) {
      return;
    }

    error = '';
    message = '';
    try {
      const skill = await installSkillFromDir(selected);
      message = `已导入 Skill：${skill.name}`;
    } catch (e: any) {
      error = e?.message ?? String(e);
    }
  }

  async function toggleEnabled(skillId: string, enabled: boolean) {
    busySkillId = skillId;
    error = '';
    message = '';
    try {
      const skill = await setSkillEnabled(skillId, enabled);
      message = `${skill.name} 已${enabled ? '启用' : '停用'}`;
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      busySkillId = '';
    }
  }

  async function toggleTrusted(skillId: string, trusted: boolean) {
    busySkillId = skillId;
    error = '';
    message = '';
    try {
      const skill = await setSkillTrusted(skillId, trusted);
      message = `${skill.name} 已${trusted ? '设为信任' : '取消信任'}`;
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      busySkillId = '';
    }
  }

  async function handleRemove(skillId: string, name: string) {
    const accepted = await confirm(`删除 Skill「${name}」？`, {
      title: '删除 Skill',
      kind: 'warning',
    });
    if (!accepted) {
      return;
    }

    busySkillId = skillId;
    error = '';
    message = '';
    try {
      await removeSkill(skillId);
      message = `${name} 已删除`;
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      busySkillId = '';
    }
  }
</script>

<div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
  <div class="flex items-start justify-between gap-3">
    <button
      type="button"
      class="flex min-w-0 flex-1 items-start gap-2 text-left"
      on:click={() => (collapsed = !collapsed)}
      aria-expanded={!collapsed}
    >
      <svg
        class="mt-0.5 w-4 h-4 text-app-text-secondary shrink-0 transition-transform {collapsed
          ? ''
          : 'rotate-180'}"
        viewBox="0 0 20 20"
        fill="none"
      >
        <path
          d="m5 7.5 5 5 5-5"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        ></path>
      </svg>
      <div class="min-w-0">
        <h4 class="font-medium text-app-text">Skills</h4>
        <p class="mt-1 text-sm text-app-text-secondary">
          管理内置与本地导入的 Skills。外部 Skill 需先启用并设为信任，才会参与自动匹配。
        </p>
      </div>
    </button>
    <button
      class="shrink-0 px-3 py-2 rounded-lg bg-primary-600 hover:bg-primary-500 text-white text-sm font-medium transition-colors"
      on:click={handleImportDirectory}
      type="button"
    >
      导入目录
    </button>
  </div>

  {#if !collapsed}
  <div class="space-y-4" in:slide={{ duration: 200 }}>
  {#if message}
    <div class="rounded-lg border border-green-500/20 bg-green-500/10 px-3 py-2 text-sm text-green-400">
      {message}
    </div>
  {/if}
  {#if error}
    <div class="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-400">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="rounded-lg border border-app-border bg-app-bg px-3 py-3 text-sm text-app-text-secondary">
      正在加载 Skills...
    </div>
  {:else if !$installedSkillCatalog.length}
    <div class="rounded-lg border border-dashed border-app-border px-3 py-3 text-sm text-app-text-secondary">
      还没有可用的 Skills。
    </div>
  {:else}
    <div class="space-y-2">
      {#each $installedSkillCatalog as skill (skill.id)}
        <div class="rounded-xl border border-app-border bg-app-bg px-3 py-3">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <p class="text-sm font-medium text-app-text">{skill.name}</p>
                <span class="rounded-full border border-app-border px-2 py-0.5 text-[11px] text-app-text-secondary">
                  {skill.source_type === 'builtin' ? '内置' : '本地'}
                </span>
                <span class="rounded-full border border-app-border px-2 py-0.5 text-[11px] text-app-text-secondary">
                  {skill.trigger_mode === 'auto' ? '自动触发' : '仅手动'}
                </span>
              </div>
              <p class="mt-1 text-xs leading-relaxed text-app-text-secondary">{skill.description}</p>
              <div class="mt-2 flex flex-wrap gap-2 text-[11px]">
                <span class="rounded-full border px-2 py-0.5 {skill.enabled ? 'border-green-500/20 bg-green-500/10 text-green-400' : 'border-app-border text-app-text-secondary'}">
                  {skill.enabled ? '已启用' : '未启用'}
                </span>
                <span class="rounded-full border px-2 py-0.5 {skill.trusted ? 'border-blue-500/20 bg-blue-500/10 text-blue-400' : 'border-app-border text-app-text-secondary'}">
                  {skill.trusted ? '已信任' : '未信任'}
                </span>
              </div>
            </div>

            {#if skill.source_type === 'builtin'}
              <span class="text-[11px] text-app-text-secondary">系统 Skill</span>
            {:else}
              <div class="flex shrink-0 items-center gap-2">
                <button
                  class="rounded-lg border border-app-border px-2.5 py-1.5 text-[11px] text-app-text-secondary transition-colors hover:text-app-text"
                  type="button"
                  disabled={busySkillId === skill.id}
                  on:click={() => toggleEnabled(skill.id, !skill.enabled)}
                >
                  {skill.enabled ? '停用' : '启用'}
                </button>
                <button
                  class="rounded-lg border border-app-border px-2.5 py-1.5 text-[11px] text-app-text-secondary transition-colors hover:text-app-text"
                  type="button"
                  disabled={busySkillId === skill.id}
                  on:click={() => toggleTrusted(skill.id, !skill.trusted)}
                >
                  {skill.trusted ? '取消信任' : '设为信任'}
                </button>
                <button
                  class="rounded-lg border border-red-500/20 bg-red-500/10 px-2.5 py-1.5 text-[11px] text-red-400 transition-colors hover:bg-red-500/20"
                  type="button"
                  disabled={busySkillId === skill.id}
                  on:click={() => handleRemove(skill.id, skill.name)}
                >
                  删除
                </button>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
  </div>
  {/if}
</div>
