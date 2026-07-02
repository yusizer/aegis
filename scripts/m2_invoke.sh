#!/bin/bash
# M2 invoke: init + register_compliance (on-chain Groth16 verify) + is_cleared.
set +e
. "$HOME/.cargo/env" 2>/dev/null || true
ST=$HOME/stellar-cli-bin/stellar
. /mnt/c/zk/aegis/staging/proof_m2.env
R=$REGISTRY_ID
echo "registry=$R verifier=$VERIFIER_ID"
echo "alice=$ALICE_G"

echo "=== init --help (arg format) ==="
$ST contract invoke --id "$R" --source-account alice --network local -- init --help 2>&1 | head -30

echo "=== INIT ==="
$ST contract invoke --id "$R" --source-account alice --network local --send yes -- \
  init --admin "$ALICE_G" --verifier_id "$VERIFIER_ID" \
       --image_id "$IMAGE_ID" --allow_set_root "$ROOT" --ttl_ledgers 1000 2>&1 | tail -20
echo "init rc=$?"

echo "=== REGISTER_COMPLIANCE (on-chain Groth16 verify) ==="
$ST contract invoke --id "$R" --source-account alice --network local --send yes -- \
  register_compliance --wallet "$ALICE_G" --journal "$JOURNAL" \
       --seal "$SEAL" --image_id "$IMAGE_ID" 2>&1 | tail -30
echo "register rc=$?"

echo "=== IS_CLEARED ==="
$ST contract invoke --id "$R" --source-account alice --network local -- \
  is_cleared --wallet "$ALICE_G" 2>&1 | tail -8

echo "=== GET_CLEARANCE ==="
$ST contract invoke --id "$R" --source-account alice --network local -- \
  get_clearance --wallet "$ALICE_G" 2>&1 | tail -12
