#!/bin/bash
# M5: deploy Aegis to public Soroban TESTNET (Protocol 27) so judges can verify
# the on-chain Groth16 proof on stellar.expert. Re-proves with the testnet alice
# pubkey (the seal binds to the wallet, so a fresh key needs a fresh proof).
set +e
. "$HOME/.cargo/env" 2>/dev/null
ST=$HOME/stellar-cli-bin/stellar
NET=testnet
RPC=https://soroban-testnet.stellar.org:443
PASS="Test SDF Network ; September 2015"
EXPERT="https://stellar.expert/explorer/testnet/contract"
WASM=/mnt/c/zk/aegis/artifacts

echo "=== add network $NET (idempotent) ==="
$ST network add --rpc-url "$RPC" --network-passphrase "$PASS" $NET 2>&1 | tail -3 || true
$ST network ls 2>&1 | grep -i testnet || true

echo "=== generate + fund alice_testnet + bob_testnet ==="
$ST keys generate alice_testnet --network $NET --fund 2>&1 | tail -4
ALICE_G=$($ST keys address alice_testnet 2>&1)
echo "alice_testnet G = $ALICE_G"
$ST keys generate bob_testnet --network $NET --fund 2>&1 | tail -3
BOB_G=$($ST keys address bob_testnet 2>&1)
echo "bob_testnet G = $BOB_G"

echo "=== decode alice_testnet ed25519 pubkey (strkey 0x30 -> 32B raw) ==="
PUB=$(python3 -c "
import base64
s='$ALICE_G'
s2=s+'='*(-len(s)%8)
raw=base64.b32decode(s2)
assert raw[0]==0x30, 'not ed25519 account strkey (0x30)'
print(raw[1:33].hex())
")
echo "alice pubkey hex = $PUB"
test ${#PUB} -eq 64 || { echo "BAD PUBKEY"; exit 1; }
SECRET=$(python3 -c "print('5E'*32)")

echo "=== re-prove with testnet alice pubkey (same guest -> same image_id, fresh seal) ==="
cd ~/aegis
cargo run --release -p aegis_host -- "$PUB" "$SECRET" 2>err_t.log | tee run_t.log
SEAL=$(grep -m1 '^Seal (hex):' run_t.log | awk '{print $3}')
IMG=$(grep -m1 '^Image ID (hex):' run_t.log | awk '{print $4}')
JOURNAL=$(grep -m1 '^Journal (hex):' run_t.log | awk '{print $3}')
JDIGEST=$(grep -m1 '^Journal SHA-256 (hex):' run_t.log | awk '{print $4}')
ROOT=$(grep -m1 '^K = ' run_t.log | sed 's/.*root = //')
K=$(grep -m1 '^K = ' run_t.log | sed 's/^K = \([0-9]*\).*/\1/')
echo "IMG=$IMG  ROOT=$ROOT  K=$K  JOURNAL=${#JOURNAL} chars"
test ${#JOURNAL} -eq 218 || { echo "BAD JOURNAL LEN"; tail -20 err_t.log; exit 1; }
JW=${JOURNAL:0:64}
test "$JW" = "$PUB" || { echo "JOURNAL WALLET MISMATCH (got $JW want $PUB)"; exit 1; }

echo "=== deploy groth16_verifier to testnet ==="
V=$($ST contract deploy --wasm $WASM/groth16_verifier.wasm --source-account alice_testnet --network $NET --alias aegis_verifier_testnet 2>dv_t.log | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "verifier = $V"
test -n "$V" || { echo "V DEPLOY FAIL"; tail -25 dv_t.log; exit 1; }

echo "=== deploy compliance_registry to testnet ==="
R=$($ST contract deploy --wasm $WASM/compliance_registry.wasm --source-account alice_testnet --network $NET --alias aegis_registry_testnet 2>dr_t.log | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "registry = $R"
test -n "$R" || { echo "R DEPLOY FAIL"; tail -25 dr_t.log; exit 1; }

echo "=== init registry (admin auth + TTL bump) ==="
$ST contract invoke --id "$R" --source-account alice_testnet --network $NET --send yes -- \
  init --admin "$ALICE_G" --verifier_id "$V" --image_id "$IMG" --allow_set_root "$ROOT" --ttl_ledgers 1000 2>&1 | tail -6

echo "=== register_compliance (on-chain Groth16 verify on testnet) ==="
$ST contract invoke --id "$R" --source-account alice_testnet --network $NET --send yes -- \
  register_compliance --wallet "$ALICE_G" --journal "$JOURNAL" --seal "$SEAL" --image_id "$IMG" 2>&1 | tail -10

echo "=== is_cleared(alice) ==="
$ST contract invoke --id "$R" --source-account alice_testnet --network $NET -- \
  is_cleared --wallet "$ALICE_G" 2>&1 | tail -3

echo "=== get_clearance(alice) ==="
$ST contract invoke --id "$R" --source-account alice_testnet --network $NET -- \
  get_clearance --wallet "$ALICE_G" 2>&1 | tail -4

echo "=== deploy mock-token + mint ==="
T=$($ST contract deploy --wasm $WASM/mock_token.wasm --source-account alice_testnet --network $NET --alias aegis_token_testnet 2>dt_t.log | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "token = $T"
$ST contract invoke --id "$T" --source-account alice_testnet --network $NET --send yes -- init --admin "$ALICE_G" 2>&1 | tail -3
$ST contract invoke --id "$T" --source-account alice_testnet --network $NET --send yes -- mint --to "$ALICE_G" --amount 1000000000 2>&1 | tail -3

echo "=== CLEAN transfer_if_cleared(alice -> bob, 1000) ==="
$ST contract invoke --id "$R" --source-account alice_testnet --network $NET --send yes -- \
  transfer_if_cleared --from "$ALICE_G" --to "$BOB_G" --amount 1000 --token "$T" 2>&1 | tail -6

echo "=== stellar.expert URLs (judge-verifiable) ==="
echo "VERIFIER  : $EXPERT/$V"
echo "REGISTRY  : $EXPERT/$R"
echo "TOKEN     : $EXPERT/$T"

cat > /mnt/c/zk/aegis/staging/proof_testnet.env <<EOF
TESTNET_ALICE_G=$ALICE_G
TESTNET_BOB_G=$BOB_G
TESTNET_ALICE_PUB=$PUB
TESTNET_SEAL=$SEAL
TESTNET_IMAGE_ID=$IMG
TESTNET_JOURNAL=$JOURNAL
TESTNET_JOURNAL_DIGEST=$JDIGEST
TESTNET_ROOT=$ROOT
TESTNET_K=$K
TESTNET_VERIFIER_ID=$V
TESTNET_REGISTRY_ID=$R
TESTNET_TOKEN_ID=$T
EOF
echo "=== saved proof_testnet.env ==="
grep -E 'TESTNET_(VERIFIER|REGISTRY|TOKEN)_ID' /mnt/c/zk/aegis/staging/proof_testnet.env
