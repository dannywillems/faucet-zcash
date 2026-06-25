<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { theme } from '$lib/theme.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';

  let { children } = $props();

  onMount(() => theme.init());
</script>

<div
  class="flex min-h-screen flex-col bg-zinc-50 text-zinc-900 transition-colors dark:bg-zinc-950 dark:text-zinc-100"
>
  <header class="border-b border-zinc-200 dark:border-zinc-800">
    <div class="mx-auto flex max-w-2xl items-center justify-between px-4 py-4">
      <div class="flex items-center gap-2">
        <span
          class="inline-flex h-7 w-7 items-center justify-center rounded-md bg-amber-500 font-bold text-white"
        >
          Z
        </span>
        <span class="font-semibold">Zcash Testnet Faucet</span>
      </div>
      <ThemeToggle />
    </div>
  </header>

  <main class="mx-auto w-full max-w-2xl flex-1 px-4 py-10">
    {@render children()}
  </main>

  <footer
    class="border-t border-zinc-200 px-4 py-6 text-center text-sm text-zinc-500 dark:border-zinc-800 dark:text-zinc-400"
  >
    Testnet funds only. Have no monetary value.
  </footer>
</div>
