import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Static SPA: a single fallback document, client-rendered. Cloudflare
    // Pages serves index.html for every route.
    adapter: adapter({ fallback: 'index.html', strict: false }),
  },
};

export default config;
