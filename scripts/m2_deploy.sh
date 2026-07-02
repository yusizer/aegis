#!/bin/bash
# M2 deploy: deploy groth16_verifier + compliance_registry to local, save IDs.
set -e
. "$HOME/.cargo/env" 2>/dev/null || true
ST=$HOME/stellar-cli-bin/stellar
WASM=/mnt/c/zk/aegis/artifacts

echo "=== wait for RPC healthy ==="
for i in $(seq 1 40); do
  R=$(curl -s --max-time 5 -X POST http://localhost:8000/rpc -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}')
  if echo "$R" | grep -q '"healthy"'; then echo "RPC ready @ try $i"; break; fi
  echo "  try $i: $(echo $R | head -c 80)"
  sleep 3
done
echo "$R" | head -c 120; echo

echo "=== deploy groth16_verifier ==="
V=$($ST contract deploy --wasm $WASM/groth16_verifier.wasm \
     --source-account alice --network local --alias aegis_verifier 2>deploy_v.log \
     | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "verifier_id = $V"
test -n "$V" || { echo "DEPLOY V FAILED"; cat deploy_v.log | tail -20; exit 1; }

echo "=== deploy compliance_registry ==="
R=$($ST contract deploy --wasm $WASM/compliance_registry.wasm \
     --source-account alice --network local --alias aegis_registry 2>deploy_r.log \
     | grep -oE 'C[0-9A-Z]{55}' | head -1)
echo "registry_id = $R"
test -n "$R" || { echo "DEPLOY R FAILED"; cat deploy_r.log | tail -20; exit 1; }

# append IDs to proof_m2.env
cat >> /mnt/c/zk/aegis/staging/proof_m2.env <<EOF
VERIFIER_ID=$V
REGISTRY_ID=$R
EOF
echo "=== saved IDs ==="
grep -E 'VERIFIER_ID|REGISTRY_ID' /mnt/c/zk/aegis/staging/proof_m2.env
