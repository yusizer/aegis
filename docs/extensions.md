# Aegis — production extensions

The MVP ships **membership** proof + on-chain Groth16 seal verify + transfer
gate. The guest and registry are architected so the following extensions are
additive — each composes with the existing journal/seal shape rather than
replacing it. These are the concrete next steps toward a production compliance
coprocessor.

## 1. Selective disclosure (auditor view key)

**Goal.** Let a wallet prove *to a specific auditor* what it proved to the gate,
without revealing its counterparty graph to anyone else.

**Mechanic.** The wallet already holds a `wallet_secret` (used in the nullifier).
Derive a **view key** `vk = SHA-256("aegis_view" || wallet_secret || auditor_id)`
and add a second journal field + claim:

```
journal (extended):  [wallet][root][nullifier][k][as_of][pass][view_key]
                     109 B today    +  32 B view key  =  141 B
```

The guest proves `view_key` is correctly derived from the same `wallet_secret`
that anchors the nullifier (so the auditor's view is bound to the same
attestation the chain accepted). The auditor, given `(view_key, journal)`, can
re-derive which allow-set leaves were checked without learning `wallet_secret`
or the nullifier preimage.

**Why it composes.** The on-chain verifier still checks the same Groth16 seal
against `(image_id, journal_digest)`. The registry only adds a `view_key` field
to the `Clearance` record and a `disclose_to(auditor, view_key)` entrypoint that
emits an event. No change to the proof-verify path — the gate stays
load-bearing.

**vs a shielded pool.** In a UTXO shielded pool, selective disclosure needs a
parallel view-key registry and per-note scanning. Here it is a **single
attestation**: one proof, one view key, one auditor — the compliance coprocessor
framing is lighter because the unit of disclosure is the clearance, not each
note.

## 2. Batch seal aggregation (pushing primitives)

**Goal.** N wallets → 1 on-chain `verify_proof` call instead of N.

**Mechanic.** Today each `register_compliance` invokes the Nethermind verifier
once (one BN254 `pairing_check`, ~12M instr). For a batch of N seals, aggregate
them via a single `bn254_multi_pairing_check`:

```
batch_verify(seal_1..seal_N, image_id, journal_digest_1..digest_N)
  → one multi-pairing over N G1 points + 2N G2 points
```

Soroban's BN254 host function exposes `bn254_multi_pairing_check` (Protocol 26+).
A batch of N=8 seals is expected to verify in roughly **2-3× the cost of one**
(not 8×) — pairing setup amortizes across the batch.

**Why it composes.** The registry gains a `register_compliance_batch` entrypoint
that loops the journal-claim enforcement per wallet and calls the verifier once
with the aggregated inputs. The guest program is unchanged — aggregation is a
host/contract-side optimization. The nullifier anti-replay still runs per-wallet.

**Estimated impact.** A compliance gate that verifies proofs *frequently* (every
transfer) is exactly the access pattern where batch aggregation pays off: the
marginal cost per extra wallet in a batch drops sharply. This is the "pushing
primitives" win — using the BN254 host fn's multi-pairing directly.

## 3. Deny-set + graph reachability (non-membership)

**Goal.** Prove no counterparty is in a sanctions deny-set, and no path in the
activity graph reaches a sanctioned address.

**Mechanic.** The guest today proves **membership** of K counterparties in the
allow-set. Add a parallel **non-membership** proof against a deny-set Merkle
tree (different root, also SHA-256/Poseidon2-committed):

```
for each counterparty c in graph:
    prove c ∈ allow_set_root           (membership, already done)
    prove c ∉ deny_set_root            (non-membership, new)
for each edge (c1 → c2) in graph:
    prove no path c1 ~> sanctioned     (graph reachability, stretch)
```

Non-membership in a Merkle tree is a standard construction (the path to the
leaf's would-be position plus the sibling proof that the position is empty or a
different leaf).

**Why it composes.** The journal gains a `deny_set_root` field and a `pass`
byte that now requires *both* membership and non-membership. The registry adds a
`set_deny_set_root` admin entrypoint mirroring `set_allow_set_root`. The
on-chain verifier and Groth16 seal are unchanged — the guest just proves a
stronger statement.

**Why a zkVM wins here.** Graph reachability in a hand-rolled Noir/Circom
circuit is expensive and awkward (branching over paths). In the zkVM it is a
Rust `for` loop over edges — the same code you'd write to check reachability
outside ZK, compiled. This is the load-bearing reason the zkVM choice pays off
for compliance: the logic that gets *added* tomorrow is cheaper to write and
audit than re-rolling a circuit.

## 4. Poseidon2 host-fn swap-in

**Goal.** Cut guest user cycles ~5-10× and align with Stellar's native hash.

**Mechanic.** The demo guest uses the stock `sha2` crate (software SHA-256) for
the allow-set Merkle tree. Stellar Protocol 25+ exposes **Poseidon2** as a host
function. Swapping the tree hash from SHA-256 to Poseidon2 is a one-line change
in the guest's `sha256_pair` → `poseidon2_pair`, plus passing the Poseidon2
crates through `ExecutorEnv`. The journal layout, nullifier construction, and
on-chain verifier are unchanged (the nullifier stays SHA-256, or also moves to
Poseidon2 — a choice).

**Estimated impact.** SHA-256 in software is the dominant cycle cost today.
Poseidon2 over the RISC Zero accelerator is expected to drop user cycles from
~105k toward ~15-25k, and the tree becomes native to Stellar's host fns.

## 5. Counterparty-set completeness (ASP-countersigned commitment)

**Goal.** Prove the wallet disclosed *all* its counterparties, not just a clean
subset.

**Mechanic (honest gap today).** The guest proves "every one of the K
counterparties I disclosed is in the allow-set". It does *not* prove "I
disclosed all my counterparties". A wallet could omit a tainted counterparty
from its disclosure.

**Production shape.** The ASP countersigns a **commitment to the wallet's real
on-chain activity graph** (e.g. a Merkle root of the wallet's counterparties
derived from on-chain events over `[as_of_block-W, as_of_block]`). The guest
then proves "the K counterparties I check are exactly the leaves of the
ASP-countersigned commitment". The commitment root becomes a public input
alongside `allow_set_root`, and the registry enforces the commitment matches the
ASP's published root for that wallet + window.

**Why this is the main extension.** It closes the one real soundness gap. Every
other property (nullifier, wallet binding, image_id, root binding, k>0) is
already enforced; this is specifically about *which* counterparties the prover
chooses to disclose.

---

## How these stack

Each extension is **independent and additive** — none requires re-architecting
the gate. The compliance coprocessor framing is precisely that: the guest proves
a stronger statement over time (membership → +non-membership → +completeness),
the registry enforces more public claims, and the on-chain Groth16 verify stays
a single BN254 pairing. That is the structural advantage of a zkVM compliance
coprocessor over a one-shot shielded pool: the proof *grows* with the policy
without changing the verify cost shape.
