// Pages Function: reverse-proxy /api/* to the faucet Worker.
//
// The frontend and the Worker are on different origins (*.pages.dev vs
// *.workers.dev). A browser only sends the HTTP Basic Auth credential and the
// session cookie to the SAME origin, so calling the Worker directly would fail
// the Worker's Basic Auth and drop the cookie. Routing through this same-origin
// proxy keeps both working: the _middleware Basic Auth gate runs first, then
// this forwards the request (method, headers incl. Authorization + Cookie,
// body) to the Worker and returns its response (incl. Set-Cookie) unchanged.

interface PagesContext {
  request: Request;
  env: { WORKER_ORIGIN?: string };
}

// Public Worker URL; overridable via the WORKER_ORIGIN Pages variable.
const DEFAULT_WORKER_ORIGIN =
  'https://faucet-zcash-api.be-danny-willems.workers.dev';

export async function onRequest(context: PagesContext): Promise<Response> {
  const origin = context.env.WORKER_ORIGIN ?? DEFAULT_WORKER_ORIGIN;
  const url = new URL(context.request.url);
  const target = `${origin}${url.pathname}${url.search}`;
  return fetch(new Request(target, context.request));
}
