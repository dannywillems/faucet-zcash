<script lang="ts">
  import { onMount } from 'svelte';
  import { api, ApiError, type DripResult, type FaucetBalance } from '$lib/api';
  import { validateAddress, type AddressCheck } from '$lib/wasm-validator';

  // Faucet reserves, fetched on load (public; behind the access gate).
  let balance = $state<FaucetBalance | null>(null);

  // Format zatoshis as TAZ (testnet ZEC); 1 TAZ = 100_000_000 zat.
  function taz(zat: number): string {
    return (zat / 100_000_000).toLocaleString(undefined, {
      maximumFractionDigits: 8,
    });
  }

  type Step = 'email' | 'otp' | 'faucet';

  let step = $state<Step>('email');
  let email = $state('');
  let code = $state('');
  let address = $state('');

  // True while we check (on load) whether a session cookie is already valid.
  let checking = $state(true);
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');

  // On load, ask the backend whether the httpOnly session cookie is valid.
  // The SPA can't read the cookie itself (by design), so the server is the
  // source of truth for login state.
  onMount(async () => {
    try {
      const status = await api.status();
      email = status.email;
      step = 'faucet';
    } catch {
      // Not signed in (401) or unreachable; stay on the email step.
    } finally {
      checking = false;
    }
    // Show the faucet's reserves (best effort; unavailable if the signer is
    // unreachable, e.g. before the tunnel is configured).
    try {
      balance = await api.balance();
    } catch {
      balance = null;
    }
  });

  async function logout() {
    busy = true;
    try {
      await api.logout();
    } catch {
      // Best effort; the cookie is cleared server-side and on response.
    } finally {
      busy = false;
      reset();
    }
  }

  let check = $state<AddressCheck | null>(null);
  let drip = $state<DripResult | null>(null);

  const explorerBase =
    import.meta.env.VITE_EXPLORER_BASE ??
    'https://testnet.zcashexplorer.app/transactions/';

  const addressValid = $derived(check?.valid === true);

  async function sendOtp() {
    error = '';
    notice = '';
    busy = true;
    try {
      await api.sendOtp(email.trim());
      step = 'otp';
      notice = `We sent a code to ${email.trim()}.`;
    } catch (e) {
      error = e instanceof ApiError ? e.message : 'Something went wrong.';
    } finally {
      busy = false;
    }
  }

  async function verifyOtp() {
    error = '';
    busy = true;
    try {
      await api.verifyOtp(email.trim(), code.trim());
      step = 'faucet';
      notice = '';
    } catch (e) {
      error = e instanceof ApiError ? e.message : 'Something went wrong.';
    } finally {
      busy = false;
    }
  }

  async function onAddressInput() {
    drip = null;
    const value = address.trim();
    if (!value) {
      check = null;
      return;
    }
    try {
      check = await validateAddress(value);
    } catch {
      check = { valid: false, error: 'Could not validate address.' };
    }
  }

  async function requestDrip() {
    error = '';
    busy = true;
    try {
      drip = await api.drip(address.trim());
    } catch (e) {
      error = e instanceof ApiError ? e.message : 'Something went wrong.';
    } finally {
      busy = false;
    }
  }

  function reset() {
    step = 'email';
    email = '';
    code = '';
    address = '';
    check = null;
    drip = null;
    error = '';
    notice = '';
  }
</script>

<section class="space-y-6">
  <div class="space-y-2">
    <h1 class="text-2xl font-semibold">Get testnet ZEC</h1>
    <p class="text-zinc-600 dark:text-zinc-400">
      Verify your email, paste a Zcash testnet address (transparent or unified
      with an Orchard receiver), and receive 1 TAZ.
    </p>
  </div>

  <div
    class="rounded-lg border border-zinc-200 bg-white p-4 text-sm shadow-sm dark:border-zinc-800 dark:bg-zinc-900"
  >
    <div class="mb-3 font-medium">Faucet reserves</div>
    {#if balance}
      <dl class="grid grid-cols-2 gap-4">
        <div>
          <dt class="text-zinc-500 dark:text-zinc-400">Shielded (Orchard)</dt>
          <dd class="font-mono">
            {taz(balance.orchard_spendable_zat)} TAZ
            <span class="text-zinc-500 dark:text-zinc-400">spendable</span>
          </dd>
          <dd class="font-mono text-xs text-zinc-500 dark:text-zinc-400">
            {taz(balance.orchard_total_zat)} TAZ total
          </dd>
        </div>
        <div>
          <dt class="text-zinc-500 dark:text-zinc-400">Transparent</dt>
          <dd class="font-mono">{taz(balance.transparent_total_zat)} TAZ</dd>
          <dd class="text-xs text-zinc-500 dark:text-zinc-400">
            shielded automatically before sending
          </dd>
        </div>
      </dl>
    {:else}
      <p class="text-zinc-500 dark:text-zinc-400">
        Reserves are currently unavailable.
      </p>
    {/if}
  </div>

  {#if error}
    <div
      class="rounded-md border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-900 dark:bg-red-950 dark:text-red-200"
      role="alert"
    >
      {error}
    </div>
  {/if}
  {#if notice}
    <div
      class="rounded-md border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200"
    >
      {notice}
    </div>
  {/if}

  <div
    class="rounded-lg border border-zinc-200 bg-white p-6 shadow-sm dark:border-zinc-800 dark:bg-zinc-900"
  >
    {#if checking}
      <p class="text-center text-sm text-zinc-500 dark:text-zinc-400">
        Checking your session...
      </p>
    {:else if step === 'email'}
      <form
        class="space-y-4"
        onsubmit={e => {
          e.preventDefault();
          sendOtp();
        }}
      >
        <label class="block">
          <span class="mb-1 block text-sm font-medium">Email</span>
          <input
            type="email"
            bind:value={email}
            required
            placeholder="you@example.com"
            class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm focus:border-amber-500 focus:outline-none focus:ring-1 focus:ring-amber-500 dark:border-zinc-700 dark:bg-zinc-950"
          />
        </label>
        <button
          type="submit"
          disabled={busy || !email}
          class="w-full rounded-md bg-amber-500 px-4 py-2 font-medium text-white transition-colors hover:bg-amber-600 focus:outline-none focus:ring-2 focus:ring-amber-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {busy ? 'Sending...' : 'Send code'}
        </button>
      </form>
    {:else if step === 'otp'}
      <form
        class="space-y-4"
        onsubmit={e => {
          e.preventDefault();
          verifyOtp();
        }}
      >
        <label class="block">
          <span class="mb-1 block text-sm font-medium">Verification code</span>
          <input
            inputmode="numeric"
            bind:value={code}
            required
            placeholder="123456"
            class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm tracking-widest focus:border-amber-500 focus:outline-none focus:ring-1 focus:ring-amber-500 dark:border-zinc-700 dark:bg-zinc-950"
          />
        </label>
        <button
          type="submit"
          disabled={busy || !code}
          class="w-full rounded-md bg-amber-500 px-4 py-2 font-medium text-white transition-colors hover:bg-amber-600 focus:outline-none focus:ring-2 focus:ring-amber-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {busy ? 'Verifying...' : 'Verify'}
        </button>
        <button
          type="button"
          onclick={reset}
          class="w-full text-sm text-zinc-500 hover:underline"
        >
          Use a different email
        </button>
      </form>
    {:else}
      <div
        class="mb-4 flex items-center justify-between border-b border-zinc-200 pb-3 text-sm dark:border-zinc-800"
      >
        <span class="text-zinc-600 dark:text-zinc-400">
          Signed in as <span class="font-medium">{email}</span>
        </span>
        <button
          type="button"
          onclick={logout}
          disabled={busy}
          class="text-zinc-500 hover:text-zinc-900 hover:underline disabled:opacity-50 dark:hover:text-white"
        >
          Log out
        </button>
      </div>
      <form
        class="space-y-4"
        onsubmit={e => {
          e.preventDefault();
          requestDrip();
        }}
      >
        <label class="block">
          <span class="mb-1 block text-sm font-medium">
            Zcash testnet address
          </span>
          <input
            bind:value={address}
            oninput={onAddressInput}
            required
            placeholder="tm... or utest1..."
            class="w-full rounded-md border border-zinc-300 bg-white px-3 py-2 font-mono text-sm focus:border-amber-500 focus:outline-none focus:ring-1 focus:ring-amber-500 dark:border-zinc-700 dark:bg-zinc-950"
          />
        </label>

        {#if check && !check.valid}
          <p class="text-sm text-red-600 dark:text-red-400">{check.error}</p>
        {:else if check?.valid}
          <p class="text-sm text-green-600 dark:text-green-400">
            Valid testnet address ({check.pool}).
          </p>
        {/if}

        <button
          type="submit"
          disabled={busy || !addressValid}
          class="w-full rounded-md bg-amber-500 px-4 py-2 font-medium text-white transition-colors hover:bg-amber-600 focus:outline-none focus:ring-2 focus:ring-amber-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {busy ? 'Sending funds...' : 'Request 1 TAZ'}
        </button>
      </form>

      {#if drip}
        <div
          class="mt-4 space-y-1 rounded-md border border-green-300 bg-green-50 px-4 py-3 text-sm dark:border-green-900 dark:bg-green-950"
        >
          <p class="font-medium text-green-800 dark:text-green-200">
            Sent {(drip.amount_zat / 100_000_000).toFixed(8)} TAZ ({drip.pool}).
          </p>
          <a
            href={`${explorerBase}${drip.txid}`}
            target="_blank"
            rel="noopener noreferrer"
            class="break-all font-mono text-amber-700 hover:underline dark:text-amber-400"
          >
            {drip.txid}
          </a>
        </div>
      {/if}
    {/if}
  </div>

  <details
    class="rounded-lg border border-zinc-200 bg-white p-4 text-sm text-zinc-600 shadow-sm dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400"
  >
    <summary
      class="cursor-pointer font-medium text-zinc-900 dark:text-zinc-100"
    >
      How it works
    </summary>
    <div class="mt-3 space-y-3">
      {#snippet srcLink(path: string)}
        <a
          href={`https://github.com/dannywillems/faucet-zcash/blob/main/${path}`}
          target="_blank"
          rel="noopener noreferrer"
          class="whitespace-nowrap font-mono text-xs text-amber-700 hover:underline dark:text-amber-400"
        >
          [code]
        </a>
      {/snippet}
      <p>
        This faucet runs the official Rust Zcash stack (librustzcash, Orchard
        0.14). It is non-custodial to you: you only enter a destination address.
        The steps (each links to its source):
      </p>
      <ol class="list-decimal space-y-2 pl-5">
        <li>
          <span class="font-medium">Access gate.</span> The whole site is behind
          HTTP Basic Auth to keep bots out. {@render srcLink(
            'frontend/functions/_middleware.ts',
          )}
        </li>
        <li>
          <span class="font-medium">Email verification.</span> You request a
          one-time code, sent by email (Resend); verifying it opens a session. {@render srcLink(
            'worker/src/lib.rs',
          )}
        </li>
        <li>
          <span class="font-medium">Address validation.</span> Your address is
          checked in your browser by Rust compiled to WebAssembly (the same
          <code>zcash_address</code> logic the backend uses), then re-validated
          server-side. Transparent and Orchard unified addresses are accepted;
          Sapling is not supported.
          {@render srcLink('crates/faucet-addr-wasm/src/lib.rs')}
        </li>
        <li>
          <span class="font-medium">Building the transaction.</span> A backend
          signer holds the faucet seed and uses
          <code>zcash_client_sqlite</code> to track the faucet's notes and
          nullifiers. It selects inputs, then builds, proves (Orchard via halo2;
          transparent via secp256k1) and signs the transaction. The browser and
          the edge Worker never see the seed. {@render srcLink(
            'signer/src/wallet.rs',
          )}
        </li>
        <li>
          <span class="font-medium">Automatic shielding.</span> The faucet is
          funded by mining rewards, which arrive as transparent coinbase. Those
          cannot be sent directly, so the signer automatically shields them into
          the Orchard pool (after the 100-block coinbase maturity) before
          dripping. That is why the reserves above show a transparent balance
          that becomes spendable shielded funds. {@render srcLink(
            'deploy/faucet-maintenance.sh',
          )}
        </li>
        <li>
          <span class="font-medium">Broadcast.</span> The signed transaction is
          broadcast to a Zcash testnet node over the lightwalletd protocol, and
          the resulting txid is shown to you with an explorer link. {@render srcLink(
            'signer/src/wallet.rs',
          )}
        </li>
      </ol>
      <p class="font-medium text-zinc-900 dark:text-zinc-100">
        Background jobs
      </p>
      <ul class="list-disc space-y-2 pl-5">
        <li>
          <span class="font-medium">Signer service</span> (always on): a native
          Rust process on the faucet host that holds the seed, stays synced to
          the chain, and builds/proves/broadcasts transactions.
          {@render srcLink('signer/src/main.rs')}
        </li>
        <li>
          <span class="font-medium">Maintenance job</span> (every 10 minutes): a
          cron on the faucet host that shields matured coinbase into Orchard and
          refreshes the reserves shown above. {@render srcLink(
            'deploy/faucet-maintenance.sh',
          )}
        </li>
      </ul>
      <p>
        Full source:
        <a
          href="https://github.com/dannywillems/faucet-zcash"
          target="_blank"
          rel="noopener noreferrer"
          class="text-amber-700 hover:underline dark:text-amber-400"
        >
          github.com/dannywillems/faucet-zcash
        </a>.
      </p>
    </div>
  </details>
</section>
