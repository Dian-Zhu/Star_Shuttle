<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  
  const dispatch = createEventDispatcher<{
    close: void;
    minimize: void;
    quit: void;
  }>();
  
  function handleMinimize() {
    dispatch('minimize');
  }
  
  function handleQuit() {
    dispatch('quit');
  }
  
  function handleCancel() {
    dispatch('close');
  }
</script>

<div class="fixed inset-0 z-[1001] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true">
  <div class="bg-app-surface border border-app-border p-[30px] rounded-xl w-[90%] max-w-[400px] shadow-2xl">
    <div class="text-base font-semibold text-app-text mb-[15px]">关闭应用</div>
    <div class="text-sm text-app-text-secondary mb-[20px] leading-relaxed">
      您希望将应用最小化到系统托盘继续运行，还是直接退出程序？
    </div>
    
    <div class="flex justify-end gap-[10px]">
      <button 
        class="px-4 py-2 border border-app-border bg-app-bg rounded-lg text-sm text-app-text cursor-pointer hover:bg-app-bg-hover transition-colors"
        on:click={handleCancel}
      >
        取消
      </button>
      
      <button 
        class="px-4 py-2 border border-app-border bg-app-bg rounded-lg text-sm text-app-text cursor-pointer hover:bg-app-bg-hover transition-colors"
        on:click={handleQuit}
      >
        退出应用
      </button>

      <button 
        class="px-4 py-2 border border-transparent bg-primary-600 rounded-lg text-sm text-white cursor-pointer hover:bg-primary-500 transition-colors shadow-lg shadow-primary-900/20"
        on:click={handleMinimize}
      >
        最小化
      </button>
    </div>
  </div>
</div>
