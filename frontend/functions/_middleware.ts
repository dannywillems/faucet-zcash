// Cloudflare Pages Function: HTTP Basic Auth gate over the whole site (the
// anti-bot filter). The credential is the same base64("user:pass") secret the
// Worker checks, so the browser sends it to the same-origin /api routes too.
//
// This file runs in the Pages Functions runtime, not in the SvelteKit app, so
// it is intentionally outside src/ and uses only Web platform types.

interface PagesContext {
  request: Request;
  env: { BASIC_AUTH_B64?: string };
  next: () => Promise<Response>;
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

function unauthorized(): Response {
  return new Response('Unauthorized', {
    status: 401,
    headers: { 'WWW-Authenticate': 'Basic realm="faucet"' },
  });
}

export async function onRequest(context: PagesContext): Promise<Response> {
  const expected = context.env.BASIC_AUTH_B64;
  // Fail closed if the gate is not configured.
  if (!expected) return unauthorized();

  const header = context.request.headers.get('Authorization') ?? '';
  const provided = header.startsWith('Basic ') ? header.slice(6) : '';
  if (!provided || !timingSafeEqual(provided, expected)) {
    return unauthorized();
  }
  return context.next();
}
