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
