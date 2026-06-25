// Dark/light theme store (Svelte 5 runes). The initial class is applied by an
// inline script in app.html to avoid a flash; this keeps the toggle in sync.

class Theme {
  dark = $state(false);

  init() {
    if (typeof document === 'undefined') return;
    this.dark = document.documentElement.classList.contains('dark');
  }

  toggle() {
    this.dark = !this.dark;
    document.documentElement.classList.toggle('dark', this.dark);
    try {
      localStorage.setItem('theme', this.dark ? 'dark' : 'light');
    } catch {
      // Ignore storage failures (private mode, etc.).
    }
  }
}

export const theme = new Theme();
