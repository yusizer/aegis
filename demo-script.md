# Aegis — 2-3 min demo script

Record screen (terminal). Keep the quickstart container + WSL holder running
before recording (`scripts/m2_restart_p27.sh` then `scripts/m4_demo.sh` already
leaves the network up and contracts deployed).

## 0. Hook (10s)
> "Stablecoins need compliance. Users need privacy. Aegis proves a wallet's funds
> are clean — in zero knowledge — and a Soroban contract gates transfers on that
> proof."

## 1. Architecture (15s) — show README mermaid diagram
> "Off-chain, a RISC Zero guest proves counterparties are in a screened allow-set
> without revealing them. On-chain, Soroban verifies the Groth16 seal and stores
> a clearance. Transfers only flow if cleared."

## 2. Off-chain proof (30s)
```
cd ~/aegis && cargo run --release -p aegis_host -- <alice_pubkey_hex> 5E5E…5E
```
- Point to output: `Seal (hex): 73c457ba…`, `Image ID`, `Journal (hex)` (109B),
  `Journal SHA-256`, and the `STATS` line (total 262,144 / user 104,956 cycles).
> "One segment, ~105k user cycles. The seal is selector-prefixed for the on-chain
> verifier."

## 3. On-chain verify (40s)
```
# deploy + init already done; show register_compliance:
stellar contract invoke --id aegis_registry --source-account alice \
  --network local --send yes -- \
  register_compliance --wallet <alice_G> --journal <journal_hex> \
  --seal <seal_hex> --image_id <image_id_hex>
```
- Show `✅ Transaction submitted successfully!`
> "The Groth16 seal is verified on Soroban by the Nethermind BN254 verifier.
> Alice is now cleared."

## 4. Clean transfer succeeds (20s)
```
stellar contract invoke --id aegis_registry --source-account alice \
  --network local --send yes -- \
  transfer_if_cleared --from <alice_G> --to <bob_G> --amount 1000 --token <token_C>
# balances:
… balance --addr <alice_G>   # 999999000
… balance --addr <bob_G>     # 1000
```
> "Cleared wallet → transfer flows through the gate."

## 5. Blocked transfer reverts (25s)
```
stellar contract invoke --id aegis_registry --source-account bob \
  --network local --send yes -- \
  transfer_if_cleared --from <bob_G> --to <alice_G> --amount 1 --token <token_C>
```
- Show `❌ HostError: Error(Contract, #9)` = `NotCleared`.
> "Bob has no proof — no clearance — the gate blocks the transfer. ZK is
> load-bearing: there is no other path to cleared."

## 6. Close (10s)
> "Aegis — provable clean funds on Stellar. Repo link in the description."
