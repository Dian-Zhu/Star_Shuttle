<script lang="ts">
  import { showAdvancedModal } from '../lib/store';
  import XIcon from './icons/XIcon.svelte';

  function handleClose() {
    showAdvancedModal.set(false);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-slate-900 border border-slate-800 rounded-xl shadow-2xl w-full max-w-3xl h-[600px] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900">
      <h2 class="text-lg font-semibold text-slate-100">高级功能</h2>
      <button 
        class="text-slate-400 hover:text-white transition-colors p-1 rounded-md hover:bg-slate-800"
        on:click={handleClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 p-6 flex items-center justify-center text-slate-500">
      <div class="text-center">
        <svg class="w-16 h-16 mx-auto mb-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.384-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z"></path></svg>
        <h3 class="text-lg font-medium text-slate-300 mb-2">高级功能开发中</h3>
        <p class="max-w-md mx-auto">隧道管理、端口转发监控等高级功能将在此处提供。</p>
      </div>
    </div>
  </div>
</div>