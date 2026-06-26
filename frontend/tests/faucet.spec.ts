import { expect, test } from '@playwright/test';

test('renders the faucet landing and email step', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle(/Zcash Testnet Faucet/i);
  await expect(
    page.getByRole('heading', { name: /Get testnet ZEC/i }),
  ).toBeVisible();
  await expect(page.getByPlaceholder(/you@example.com/i)).toBeVisible();
});

test('theme toggle switches dark mode', async ({ page }) => {
  await page.goto('/');
  const html = page.locator('html');
  const before = await html.evaluate(el => el.classList.contains('dark'));
  await page.getByRole('button', { name: /switch to .* theme/i }).click();
  const after = await html.evaluate(el => el.classList.contains('dark'));
  expect(after).toBe(!before);
});

test('rejects an invalid address client-side via wasm', async ({ page }) => {
  // Advance to the address step by stubbing the OTP endpoints so no real
  // backend is needed (the wasm validation under test is client-side).
  await page.route('**/api/auth/send-otp', route =>
    route.fulfill({ status: 200, body: JSON.stringify({ message: 'ok' }) }),
  );
  await page.route('**/api/auth/verify-otp', route =>
    route.fulfill({ status: 200, body: JSON.stringify({ message: 'ok' }) }),
  );

  await page.goto('/');
  await page.getByPlaceholder(/you@example.com/i).fill('user@example.com');
  await page.getByRole('button', { name: /send code/i }).click();
  await page.getByPlaceholder('123456').fill('123456');
  await page.getByRole('button', { name: /^verify$/i }).click();

  const addr = page.getByPlaceholder(/tm\.\.\./i);
  await expect(addr).toBeVisible();
  await addr.fill('not-a-zcash-address');
  await expect(page.getByText(/not a valid zcash address/i)).toBeVisible();
});

test('shows the background services status card', async ({ page }) => {
  await page.route('**/api/faucet/services', route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        checked_at: 1_700_000_000,
        services: [
          {
            key: 'worker',
            name: 'Worker API',
            status: 'ok',
            detail: 'Responding',
            description: 'The faucet HTTP API, a Rust Cloudflare Worker.',
            code_path: 'worker/src/lib.rs',
            endpoints: ['GET /api/health', 'POST /api/faucet/drip'],
          },
          {
            key: 'signer',
            name: 'Signer service',
            status: 'down',
            detail: 'Unreachable over the tunnel',
            description: 'Native Rust process on the faucet host.',
            code_path: 'signer/src/wallet.rs',
          },
          {
            key: 'heartbeat',
            name: 'Heartbeat job',
            status: 'degraded',
            detail: 'Last self-send 20m ago (txid abcd012345...)',
            description: 'A Cloudflare Worker Cron Trigger (every 5 minutes).',
            code_path: 'worker/src/lib.rs',
          },
        ],
      }),
    }),
  );

  await page.goto('/');
  await expect(page.getByText('Background services')).toBeVisible();
  await expect(page.getByText('Worker API')).toBeVisible();
  await expect(page.getByText('Operational')).toBeVisible();
  await expect(page.getByText('Down')).toBeVisible();
  await expect(page.getByText('Degraded')).toBeVisible();
  // The Worker API lists its actual endpoints.
  await expect(page.getByText('POST /api/faucet/drip')).toBeVisible();
  // Each service links to its source.
  await expect(page.getByRole('link', { name: 'code' }).first()).toBeVisible();
});
