<script lang="ts">
  import { theme } from '$lib/theme.svelte';
</script>

<button
  type="button"
  onclick={() => theme.toggle()}
  class="rounded-md p-2 text-zinc-600 transition-colors hover:bg-zinc-100 hover:text-zinc-900 focus:outline-none focus:ring-2 focus:ring-amber-500 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
  aria-label={theme.dark ? 'Switch to light theme' : 'Switch to dark theme'}
>
  {#if theme.dark}
    <!-- sun -->
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      class="h-5 w-5"
    >
      <circle cx="12" cy="12" r="4" />
      <path
        d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"
      />
    </svg>
  {:else}
    <!-- moon -->
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      class="h-5 w-5"
    >
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  {/if}
</button>
