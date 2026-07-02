#!/bin/bash
# M4 E2E demo: build mock-token, deploy, mint, transfer_if_cleared (clean) + blocked (tainted).
set +e
. "$HOME/.cargo/env" 2>/dev/null || true
ST=$HOME/stellar-cli-bin/stellar
. /mnt/c/zk/aegis/staging/proof_m2.env
R=$REGISTRY_ID
BOB_G=$($ST keys address bob 2>&1)
echo "bob=$BOB_G"

echo "=== build mock-token ==="
cd /mnt/c/zk/aegis/contracts/mock-token
cargo build --release --target wasm32v1-none 2>&1 | tail -8
W=$(find target/wasm32v1-none/release -name "mock_token.wasm" | head -1)
echo "wasm = $W"
test -n "$W" || { echo "BUILD FAILED"; exit 1; }
cp "$W" /mnt/c/zk/aegis/artifacts/mock_token.wasm
ls -la /mnt/c/zk/aegis/artifacts/mock_token.wasm

echo "=== wait RPC ==="
for i in $(seq 1 30); do
  H=$(curl -s --max-time 5 -X POST http://localhost:8000/rpc -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}')
  echo "$H" | grep -q '"healthy"' && { echo "RPC ready @ $i"; break; }
  sleep 3
done

echo "=== deploy mock-token ==="
T=$($ST contract deploy --wasm /mnt/c/zk/aegis/artifacts/mock_token.wasm \
     --source-account alice --network local --alias aegis_token 2>deploy_t.log \
     | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "token_id = $T"
test -n "$T" || { echo "TOKEN DEPLOY FAILED"; cat deploy_t.log | tail -20; exit 1; }

echo "=== init + mint ==="
$ST contract invoke --id "$T" --source-account alice --network local --send yes -- \
  init --admin "$ALICE_G" 2>&1 | tail -3
$ST contract invoke --id "$T" --source-account alice --network local --send yes -- \
  mint --to "$ALICE_G" --amount 1000000000 2>&1 | tail -3

echo "=== balances before ==="
$ST contract invoke --id "$T" --source-account alice --network local -- balance --addr "$ALICE_G" 2>&1 | tail -2
$ST contract invoke --id "$T" --source-account alice --network local -- balance --addr "$BOB_G" 2>&1 | tail -2

echo "=== CLEAN PATH: transfer_if_cleared(alice -> bob, 1000) ==="
$ST contract invoke --id "$R" --source-account alice --network local --send yes -- \
  transfer_if_cleared --from "$ALICE_G" --to "$BOB_G" --amount 1000 --token "$T" 2>&1 | tail -6

echo "=== balances after (expect alice=999999000, bob=1000) ==="
$ST contract invoke --id "$T" --source-account alice --network local -- balance --addr "$ALICE_G" 2>&1 | tail -2
$ST contract invoke --id "$T" --source-account alice --network local -- balance --addr "$BOB_G" 2>&1 | tail -2

echo "=== BLOCKED PATH: is_cleared(bob) then transfer_if_cleared(bob -> alice, 1) ==="
$ST contract invoke --id "$R" --source-account bob --network local -- is_cleared --wallet "$BOB_G" 2>&1 | tail -2
echo "--- attempt transfer (expect NotCleared revert) ---"
$ST contract invoke --id "$R" --source-account bob --network local --send yes -- \
  transfer_if_cleared --from "$BOB_G" --to "$ALICE_G" --amount 1 --token "$T" 2>&1 | tail -12

# save token id
echo "TOKEN_ID=$T" >> /mnt/c/zk/aegis/staging/proof_m2.env
echo "=== done, TOKEN_ID=$T ==="
