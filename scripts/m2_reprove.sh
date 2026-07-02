#!/bin/bash
# M2 re-prove: patch host with CLI args, decode alice pubkey, re-prove, save outputs.
set -e
set -o pipefail
. "$HOME/.cargo/env" 2>/dev/null || true
ST=/home/yus23/stellar-cli-bin/stellar
ALICE_G=$($ST keys address alice 2>&1)
echo "alice G = $ALICE_G"

echo "=== patch host main.rs ==="
cp /mnt/c/zk/aegis/staging/host_main_v2.rs ~/aegis/host/src/main.rs
head -5 ~/aegis/host/src/main.rs

echo "=== decode alice pubkey (strkey -> 32B raw) ==="
PUB=$(python3 -c "
import base64, sys
s='$ALICE_G'
s2=s+'='*(-len(s)%8)
raw=base64.b32decode(s2)
assert raw[0]==0x30, 'not ed25519 account strkey (0x30)'
print(raw[1:33].hex())
")
echo "alice pubkey hex = $PUB"
test ${#PUB} -eq 64 || { echo "BAD PUBKEY LEN"; exit 1; }

SECRET=$(python3 -c "print('5E'*32)")
echo "secret hex = $SECRET"

echo "=== cargo run (Groth16 prove via Docker) ==="
cd ~/aegis
cargo run --release -p aegis_host -- "$PUB" "$SECRET" 2>err.log | tee run.log

echo "=== parse outputs ==="
SEAL=$(grep -m1 '^Seal (hex):' run.log | awk '{print $3}')
IMG=$(grep -m1 '^Image ID (hex):' run.log | awk '{print $4}')
JOURNAL=$(grep -m1 '^Journal (hex):' run.log | awk '{print $3}')
JDIGEST=$(grep -m1 '^Journal SHA-256 (hex):' run.log | awk '{print $4}')
ROOT=$(grep -m1 '^K = ' run.log | sed 's/.*root = //')
K=$(grep -m1 '^K = ' run.log | sed 's/^K = \([0-9]*\).*/\1/')

echo "SEAL=${SEAL:0:40}...(${#SEAL} chars)"
echo "IMG=$IMG"
echo "JOURNAL=$JOURNAL (${#JOURNAL} chars = $((${#JOURNAL}/2)) bytes)"
echo "JDIGEST=$JDIGEST"
echo "ROOT=$ROOT K=$K"

# sanity: journal must be 109 bytes = 218 hex chars
test ${#JOURNAL} -eq 218 || { echo "BAD JOURNAL LEN: ${#JOURNAL}"; cat err.log | tail -20; exit 1; }
# sanity: wallet field in journal == alice pubkey
JWALLET=${JOURNAL:0:64}
echo "journal wallet = $JWALLET"
test "$JWALLET" = "$PUB" || { echo "JOURNAL WALLET MISMATCH"; exit 1; }

# save for deploy step
cat > /mnt/c/zk/aegis/staging/proof_m2.env <<EOF
ALICE_G=$ALICE_G
ALICE_PUB=$PUB
SEAL=$SEAL
IMAGE_ID=$IMG
JOURNAL=$JOURNAL
JOURNAL_DIGEST=$JDIGEST
ROOT=$ROOT
K=$K
EOF
echo "=== saved proof_m2.env ==="
cat /mnt/c/zk/aegis/staging/proof_m2.env
