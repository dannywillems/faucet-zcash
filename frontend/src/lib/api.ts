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
    throw new ApiError(0, 'Network error. Please try again.');
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
    const message =
      (data as { error?: string } | null)?.error ??
      `Request failed (${res.status}).`;
    throw new ApiError(res.status, message);
  }
  return data as T;
}

export const api = {
  sendOtp(email: string): Promise<{ message: string }> {
    return request('/auth/send-otp', 'POST', { email });
  },
  verifyOtp(email: string, code: string): Promise<{ message: string }> {
    return request('/auth/verify-otp', 'POST', { email, code });
  },
  status(): Promise<FaucetStatus> {
    return request('/faucet/status', 'GET');
  },
  drip(address: string): Promise<DripResult> {
    return request('/faucet/drip', 'POST', { address });
  },
};
