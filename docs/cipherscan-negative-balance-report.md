# cipherscan testnet: negative transparent balance on a mining/shielding address

## Summary

The cipherscan testnet explorer reports a **negative balance** for a transparent
address that mines coinbase and then shields it into Orchard. A transparent
balance cannot be negative on-chain (a UTXO set cannot go below zero). The cause
is per-address balance accounting: shielding transactions are recorded as a plain
transparent "sent" whose value leaves for a shielded pool, and that outgoing
value is debited from the address without any offsetting credit on the
transparent ledger. Across a mine -> shield lifecycle this drives the computed
`received - sent` balance negative.

This is an explorer accounting issue, not a chain state. The address's real
transparent UTXO balance is 0.

## Affected address

```
tmUqQn2hFWTRBNSypuMDahHww6bZLr3BrN2
https://testnet.cipherscan.app/address/tmUqQn2hFWTRBNSypuMDahHww6bZLr3BrN2
```

This is a faucet mining address: it receives coinbase, and the faucet shields the
matured coinbase into the Orchard pool (Zcash coinbase must be spent to a
shielded output). So the address's only activity is "receive coinbase" then
"shield it".

## What cipherscan reports

From `GET /api/address/tmUqQn2hFWTRBNSypuMDahHww6bZLr3BrN2`:

```
balance:        -11.251496      (== totalReceived - totalSent)
totalReceived:   2184.800674
totalSent:       2196.05217
txCount:         1828
```

The `balance` is exactly `totalReceived - totalSent`, i.e. cipherscan believes the
address has sent ~11.25 TAZ more than it ever received.

The figure is not stable; it drifts as new blocks are indexed. Two reads a few
minutes apart returned `-12.501496` and then `-11.251496` (a change of exactly
1.25 TAZ, one block reward), consistent with `totalReceived` catching up to
shield spends that were already counted. A real on-chain balance would not drift
this way.

## Root cause: shields counted as transparent "sent" with no offsetting credit

A shielding transaction spends transparent coinbase UTXOs (inputs at this
address) and produces a **shielded** (Orchard) output. cipherscan records it as a
transparent send with `outputValue = 0` and `to = null`, and does not flag it as
shielded.

Concrete reproducer, a shielding transaction for this address:

```
txid:        c1ebd3acc7b352aa90b69e8f52c73f1d87e18780ad62812098f0cd789eb89a6c
blockHeight: 4093383
```

cipherscan's own API returns this object for that tx:

```json
{
  "txid": "c1ebd3acc7b352aa90b69e8f52c73f1d87e18780ad62812098f0cd789eb89a6c",
  "blockHeight": 4093383,
  "inputValue": 32.51595,
  "outputValue": 0,
  "netChange": -32.51595,
  "amount": 32.51595,
  "type": "sent",
  "isCoinbase": false,
  "isShielded": false,
  "from": "tmUqQn2hFWTRBNSypuMDahHww6bZLr3BrN2",
  "to": null
}
```

What this transaction actually is: it spends 26 transparent coinbase UTXOs
(totaling 32.51595 TAZ) from this address into a single Orchard (shielded)
output. So:

- `inputValue = 32.51595` is correct (the coinbase being spent).
- `outputValue = 0` and `to = null` are the problem: the output value (minus fee)
  went into the Orchard pool, which cipherscan does not represent on the
  transparent ledger, so it credits nothing.
- `isShielded = false` is incorrect; this is a shielding transaction (it has
  shielded outputs).

Net effect: every shield subtracts its full input value from the address's
running transparent balance with no matching credit, so an address that mines and
shields trends negative.

## Where cipherscan is correct

Coinbase receipts are credited correctly. Sampling several of this address's
coinbase transactions, the API returns `type = "received"`, `isCoinbase = true`,
and a positive `netChange` equal to the reward. The discrepancy is specific to
the shielding spends, not to coinbase credits.

## Why it cannot be a real balance

- A transparent address's spendable balance is the sum of its unspent outputs; it
  is bounded below by 0.
- `balance = totalReceived - totalSent` only equals the UTXO balance when both
  sides are accounted consistently. Here the shielded output of a shield tx is
  omitted from the transparent side, so the identity breaks and the value can go
  negative.

## Suggested fix

Treat shielding transactions consistently in per-address transparent accounting.
Options, in rough order of preference:

1. Detect transactions with shielded outputs and set `isShielded = true`; for
   per-address transparent balance, do not let a shield's transparent input
   spend reduce the address balance below the actual remaining UTXO set. The
   per-address transparent balance should be computed from the unspent
   transparent output set, not from `received - sent`.
2. If `balance` must remain `received - sent`, then also account the shielded
   output on the transparent ledger (for example as value leaving to the
   shielded pool) so the two sides net correctly, rather than dropping it.
3. As a guard, clamp/flag any per-address transparent balance that computes below
   zero, since that is definitionally impossible and indicates an accounting gap.

## Verification notes / caveats

- The address's real transparent UTXO balance is 0: a wallet that tracks it
  reports `transparent_total = 0` (all received outputs spent), and the funds are
  present in the Orchard pool.
- We could not reconcile the exact `-11.25` figure to the cent from outside: the
  public API returned only 25 of the 1828 transactions, so the full lifetime sum
  cannot be re-derived externally. The mechanism above is demonstrated by the
  single reproducer transaction and the drift behavior, which are sufficient to
  locate the accounting path at fault.

## How to reproduce

1. `GET https://testnet.cipherscan.app/api/address/tmUqQn2hFWTRBNSypuMDahHww6bZLr3BrN2`
   and observe `balance < 0` with `balance == totalReceived - totalSent`.
2. Inspect tx
   `c1ebd3acc7b352aa90b69e8f52c73f1d87e18780ad62812098f0cd789eb89a6c`: a shield
   with `outputValue = 0`, `to = null`, `isShielded = false`, contributing its
   full `inputValue` to the address's outgoing total.
3. Compare against any coinbase receipt for the same address, which is credited
   correctly, to confirm the asymmetry is specific to shields.
