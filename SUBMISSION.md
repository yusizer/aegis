# Aegis — submission kit

Ready-to-paste text for the DoraHacks BUIDL form, the demo video, and the SDF
Discord `#zk-chat` post.

## DoraHacks BUIDL — project name
`Aegis — Provable Clean-Funds Compliance Coprocessor (RISC Zero zkVM)`

## DoraHacks BUIDL — short description (one-liner)
A wallet proves — in zero knowledge — that its counterparties belong to a
screened allow-set; a Soroban contract verifies the RISC Zero Groth16 seal
on-chain and gates stablecoin transfers on that proof. Privacy preserved,
compliance provable.

## Links
- **Repo:** https://github.com/yusizer/aegis
- **Live web demo:** https://yusizer.github.io/aegis/ (interactive overview, clickable testnet contract/tx links on stellar.expert, visual demo flow, security model, comparison table)
- **Demo video:** https://youtu.be/HwHKL6Jmvqk (YouTube unlisted, 125s, voiced, title "Aegis — ZK Clean-Funds Compliance Coprocessor on Stellar (RISC Zero zkVM)")
- **Testnet contracts (judge-verifiable on stellar.expert):**
  - Verifier — https://stellar.expert/explorer/testnet/contract/CBZAX43T4YNSWNWM2GCIHUSNWUAQYMOD5RJZVLNNC3UDIG6Z6IOQUZNB
  - Registry — https://stellar.expert/explorer/testnet/contract/CAI3XYL2KRM7BCJYN46DODKGIIKMFFNFWYPNKRRWYXVE3ZIXBTGQCERB
  - Token — https://stellar.expert/explorer/testnet/contract/CDQDBN2HA64U4M3MCIDHHRQCL5XOXE5CVSZNFGBAKQ6LD5GFVNE67AMK

## DoraHacks BUIDL — full description (paste)
Aegis is a ZK compliance layer for stablecoins on Stellar. Today, compliance
(“no flow to sanctioned addresses”) and privacy (“don’t publish your
transaction graph”) conflict on a transparent ledger. Aegis resolves that with
zero knowledge:

- **Off-chain**, a RISC Zero zkVM guest (Rust, no_std) takes a wallet’s private
  counterparty graph + a SHA-256-committed ASP allow-set Merkle root (Poseidon2
  host-fn swap-in is a one-line change for Protocol 25+), and proves — without
  revealing the graph — that every one of K counterparties is a member of the
  allow-set (Merkle membership), plus a domain-separated nullifier
  `SHA-256("aegis_null" || wallet || secret || root || as_of_block)` that is
  bound to the wallet AND the ledger (two wallets can't share a nullifier; one
  wallet can't rotate its secret to bypass anti-replay). It commits a 109-byte
  journal and produces a Groth16 seal.
- **On-chain**, a Soroban `ComplianceRegistry` hashes the journal and calls the
  Nethermind `stellar-risc0-verifier` to verify the seal against
  `(image_id, journal_digest)` using BN254 host functions (Protocol 26+/27). It
  then enforces the public claims (image_id, allow_set_root, wallet binding via
  `Address::to_payload`, pass==1, nullifier non-replay) and stores a clearance
  record for a TTL window.
- **The gate**: `transfer_if_cleared(from, to, amount, token)` reverts unless
  `from` is currently cleared. The transfer gate rejects without a valid Groth16
  seal — ZK is load-bearing.

**Demo (on a Protocol 27 Soroban network):**
- Clean wallet → on-chain Groth16 verify passes → cleared →
  `transfer_if_cleared` succeeds (alice 999_999_000, bob 1_000).
- No-proof wallet → `is_cleared` false → `transfer_if_cleared` reverts
  `NotCleared` (#9). The gate holds.

**Performance:** guest runs in a single RISC Zero segment, 105,224 user cycles /
262,144 total (25,361 paging); on-chain verify is a single BN254 `pairing_check`
(~12M Soroban instr).

**Stack:** RISC Zero zkVM 3.0.x · Soroban (Protocol 27) · Nethermind
stellar-risc0-verifier · Stellar CLI v27.

This maps directly onto SDF’s “compliance-ready from the start” north-star and
the Confidential Tokens compliance layer (auditor view keys, selective
disclosure, policy engine): Aegis is the verifiable proof layer under such a
policy engine.

Hackathon prototype, not audited; the on-chain Groth16 verifier is a Stellar dev
preview. Limitations and future work (Poseidon2 swap-in, deny-set + graph
reachability, batch seal aggregation via `bn254_multi_pairing_check`) are listed
in the README.

## Demo video
`aegis-demo-voiced.mp4` (~2 min, 125s, voiced terminal walkthrough, rendered
from the **public testnet** run — real seal, image_id, journal, balances, and
stellar.expert contract IDs; end-card with repo link). Upload to YouTube as
unlisted and paste the link, or upload directly if DoraHacks allows. Generated
from `demo-video/aegis-demo-v2.html` + `demo-video/record-v2.js` (Playwright) +
`demo-video/voice/` — reproducible: `cd demo-video && node record-v2.js` → webm
→ ffmpeg → mp4 → merge voice.

## SDF Discord #zk-chat post
> Sharing my Stellar Hacks: Real-World ZK submission — **Aegis**, a ZK compliance
> coprocessor on Stellar. A RISC Zero zkVM guest proves (in zero knowledge) that
> a wallet’s counterparties are all in a screened ASP allow-set; a Soroban
> `ComplianceRegistry` verifies the Groth16 seal on-chain with the Nethermind
> BN254 verifier (Protocol 27) and gates stablecoin transfers on the proof —
> `transfer_if_cleared` reverts without a valid seal. Demo: clean wallet clears &
> transfers, no-proof wallet is blocked (`NotCleared #9`). Single-segment guest,
> ~105k user cycles. Repo: https://github.com/yusizer/aegis — would love feedback
> from the ZK crew, especially on the deny-set / graph-reachability extension and
> batch seal aggregation. 🛡️

## Submission checklist
- [x] Public open-source repo + README — https://github.com/yusizer/aegis
- [x] Live web demo — https://yusizer.github.io/aegis/
- [x] Deployed on public Stellar testnet (P27) — judge-verifiable on stellar.expert:
  - Verifier — https://stellar.expert/explorer/testnet/contract/CBZAX43T4YNSWNWM2GCIHUSNWUAQYMOD5RJZVLNNC3UDIG6Z6IOQUZNB
  - Registry — https://stellar.expert/explorer/testnet/contract/CAI3XYL2KRM7BCJYN46DODKGIIKMFFNFWYPNKRRWYXVE3ZIXBTGQCERB
  - Token — https://stellar.expert/explorer/testnet/contract/CDQDBN2HA64U4M3MCIDHHRQCL5XOXE5CVSZNFGBAKQ6LD5GFVNE67AMK
  - `register_compliance` tx (on-chain Groth16 verify) — 25f9e655798756ab8b2d1fd368f566a1ba960a9dd95a5921d26523a920fd6542
- [x] 2–3 min demo video (aegis-demo-voiced.mp4, 125s, voiced) — ready to upload
- [x] Guest-logic test suite (9 unit tests: Merkle reconstruction, tamper rejection, demo root pin)
- [x] Upload demo video to YouTube (unlisted) — https://youtu.be/HwHKL6Jmvqk
- [ ] Submit BUIDL on DoraHacks before 2026-07-03 17:00 UTC
- [ ] Post in SDF Discord #zk-chat
