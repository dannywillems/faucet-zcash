// Client-rendered SPA: no SSR (the app talks to the Worker API and loads the
// wasm validator in the browser), single fallback document.
export const ssr = false;
export const prerender = false;
