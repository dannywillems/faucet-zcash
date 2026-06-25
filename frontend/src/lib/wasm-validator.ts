// Wraps the Rust `faucet-addr-wasm` validator (generated into ./wasm by
// `make build-wasm-addr`) so the browser reuses the zcash_address logic.

import init, { validate_testnet_address } from './wasm/faucet_addr_wasm.js';

export interface AddressCheck {
  valid: boolean;
  pool?: string;
  error?: string;
}

let ready: Promise<void> | null = null;

/** Validate a destination address against testnet rules in the browser. */
export async function validateAddress(addr: string): Promise<AddressCheck> {
  if (!ready) {
    ready = init().then(() => undefined);
  }
  await ready;
  const result = validate_testnet_address(addr);
  return { valid: result.valid, pool: result.pool, error: result.error };
}
