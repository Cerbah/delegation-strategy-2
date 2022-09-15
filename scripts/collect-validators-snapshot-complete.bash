#!/bin/bash

SCRIPT_DIR=$(dirname "$0")
BIN_DIR=$SCRIPT_DIR/../target/debug

if [[ -z $WHOIS_BEARER_TOKEN ]]
then
  echo "Env variable WHOIS_BEARER_TOKEN is missing!"
  exit 1
fi

"$BIN_DIR/collect" \
  --url http://api.mainnet-beta.solana.com \
  validators \
    --with-validator-info \
    --whois "https://whois.marinade.finance" \
    --whois-bearer-token "$WHOIS_BEARER_TOKEN" \
    --escrow-relocker "tovt1VkTE2T4caWoeFP6a2xSFoew5mNpd7FWidyyMuk" \
    --gauge-meister "mvgmBamY7hDWxLNGLshMoZn8nt2P8tKnKhaBeXMVajZ"
