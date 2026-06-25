// Client for the faucet Worker API. The API shares the page origin under /api
// (production: a Worker route; dev: a Vite proxy to `wrangler dev`), so the
// session cookie and the Basic Auth credential are sent automatically.

const BASE = import.meta.env.VITE_API_BASE ?? '/api';

export type Pool = 'transparent' | 'orchard';

export interface DripResult {
  txid: string;
  pool: Pool;
  amount_zat: number;
}

export interface FaucetStatus {
  email: string;
  eligible: boolean;
  next_eligible_at: number;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function request<T>(
  path: string,
  method: 'GET' | 'POST',
  body?: unknown,
): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${BASE}${path}`, {
      method,
      credentials: 'include',
      headers: body ? { 'Content-Type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch {
    throw new ApiError(
      0,
      'Network error: could not reach the faucet API. Check your connection ' +
        'and try again.',
    );
  }

  let data: unknown = null;
  const text = await res.text();
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = null;
    }
  }

  if (!res.ok) {
    // Prefer the API's own JSON error; otherwise explain the status code.
    const serverMsg = (data as { error?: string } | null)?.error;
    const message = serverMsg ?? describeStatus(res.status, res.statusText);
    throw new ApiError(res.status, message);
  }
  return data as T;
}

/** Human-readable explanation for an HTTP status with no JSON error body. */
function describeStatus(status: number, statusText: string): string {
  switch (status) {
    case 401:
      return 'Authentication failed (401). Reload the page and re-enter the access credentials, then sign in again.';
    case 403:
      return 'Not allowed (403). This email domain may not be permitted for the faucet.';
    case 404:
    case 405:
      return `The faucet API did not handle this request (${status}). The API route is not reachable from this site - the "/api" path is not wired to the faucet backend yet.`;
    case 429:
      return 'Too many requests (429). Please wait a bit and try again.';
    case 500:
    case 502:
    case 503:
    case 504:
      return `The faucet backend is temporarily unavailable (${status}). Please try again in a few minutes.`;
    default: {
      const suffix = statusText ? ` ${statusText}` : '';
      return `Request failed (${status}${suffix}).`;
    }
  }
}

export const api = {
  sendOtp(email: string): Promise<{ message: string }> {
    return request('/auth/send-otp', 'POST', { email });
  },
  verifyOtp(email: string, code: string): Promise<{ message: string }> {
    return request('/auth/verify-otp', 'POST', { email, code });
  },
  logout(): Promise<{ message: string }> {
    return request('/auth/logout', 'POST');
  },
  status(): Promise<FaucetStatus> {
    return request('/faucet/status', 'GET');
  },
  drip(address: string): Promise<DripResult> {
    return request('/faucet/drip', 'POST', { address });
  },
};
