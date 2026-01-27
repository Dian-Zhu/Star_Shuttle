<script lang="ts">
  import Layout from './components/Layout.svelte';
  import './app.css';
  import { settings } from './lib/store';
  import { onMount } from 'svelte';

  let systemPrefersDark = true; // Default to dark

  onMount(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    systemPrefersDark = media.matches;
    
    const listener = (e: MediaQueryListEvent) => {
      systemPrefersDark = e.matches;
    };
    
    media.addEventListener('change', listener);
    return () => media.removeEventListener('change', listener);
  });

  $: effectiveTheme = $settings.theme === 'system' 
    ? (systemPrefersDark ? 'dark' : 'light') 
    : $settings.theme;

  $: {
    if (typeof document !== 'undefined') {
      document.documentElement.setAttribute('data-theme', effectiveTheme);
      if (effectiveTheme === 'dark') {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    }
  }
</script>

<Layout />
