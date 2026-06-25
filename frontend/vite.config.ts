import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    fs: {
      // Allow importing the wasm-pack output generated into src/lib/wasm.
      allow: ['..'],
    },
    // In dev, proxy the API to a local `wrangler dev` so the SPA and API share
    // an origin (matches the production /api routing).
    proxy: {
      '/api': {
        target: 'http://localhost:8787',
        changeOrigin: true,
      },
    },
  },
});
