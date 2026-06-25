import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const REPO_URL = 'https://github.com/dannywillems/faucet-zcash';

// Capture the git commit being built so the footer can show exactly what is
// deployed. Falls back to "unknown" outside a git checkout (e.g. some CI).
function gitCommit(): { short: string; full: string } {
  try {
    const full = execSync('git rev-parse HEAD').toString().trim();
    return { short: full.slice(0, 7), full };
  } catch {
    return { short: 'unknown', full: 'unknown' };
  }
}

const commit = gitCommit();
const version = JSON.parse(readFileSync('./package.json', 'utf8')).version;

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(version),
    __APP_COMMIT__: JSON.stringify(commit.short),
    __APP_COMMIT_FULL__: JSON.stringify(commit.full),
    __REPO_URL__: JSON.stringify(REPO_URL),
  },
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
