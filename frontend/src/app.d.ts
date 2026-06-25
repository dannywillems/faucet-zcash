// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
  namespace App {
    // interface Error {}
    // interface Locals {}
    // interface PageData {}
    // interface Platform {}
  }

  // Build-time constants injected by Vite `define` (see vite.config.ts).
  const __APP_VERSION__: string;
  const __APP_COMMIT__: string;
  const __APP_COMMIT_FULL__: string;
  const __REPO_URL__: string;
}

export {};
