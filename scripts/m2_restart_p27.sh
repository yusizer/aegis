#!/bin/bash
# Restart quickstart on Protocol 27 (BN254 host fns), wait, re-fund accounts.
set +e
. "$HOME/.cargo/env" 2>/dev/null || true
ST=$HOME/stellar-cli-bin/stellar

echo "=== restart quickstart on P27 ==="
docker rm -f stellar-local 2>/dev/null
docker run -d --name stellar-local --restart unless-stopped -p 8000:8000 \
  stellar/quickstart --local --limits unlimited --protocol-version 27 2>&1 | tail -2

echo "=== wait for RPC healthy + P27 ==="
PV=""
for i in $(seq 1 60); do
  R=$(curl -s --max-time 6 -X POST http://localhost:8000/rpc -H "Content-Type: application/json" \
       -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger","params":{}}')
  if echo "$R" | grep -q '"protocolVersion"'; then
    PV=$(echo "$R" | grep -oE '"protocolVersion":[0-9]+')
    H=$(curl -s --max-time 5 -X POST http://localhost:8000/rpc -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}')
    if echo "$H" | grep -q '"healthy"'; then echo "RPC ready @ try $i: $PV healthy"; break; fi
  fi
  echo "  try $i: $(echo $R | head -c 90)"
  sleep 4
done
echo "final PV=$PV"

echo "=== re-fund alice/bob/carol ==="
for k in alice bob carol; do
  A=$($ST keys address $k 2>&1)
  for t in 1 2 3 4 5 6; do
    C=$(curl -s --max-time 25 -o /dev/null -w "%{http_code}" "http://localhost:8000/friendbot?addr=$A")
    echo "  $k try$t=$C"; [ "$C" = "200" ] && break; sleep 5
  done
done

echo "=== verify via horizon ==="
for k in alice bob carol; do
  A=$($ST keys address $k 2>&1)
  C=$(curl -s --max-time 8 -o /dev/null -w "%{http_code}" "http://localhost:8000/accounts/$A")
  echo "$k: HTTP $C"
done
