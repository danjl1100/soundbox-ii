source .envrc

# fixup BEET_CMD
which "${BEET_CMD:-beet}" >/dev/null 2>&1; err=$?
if [ $err -ne 0 ]; then
  echo "Failed to find '${BEET_CMD:-beet}' in path."
  echo "*** NOTICE: Falling back to fake-beet"
  pushd ./fake-beet
  cargo build
  popd
  echo export BEET_CMD="./fake-beet/target/debug/fake-beet"
  export BEET_CMD="./fake-beet/target/debug/fake-beet"
fi

echo 'Executing cargo directly (within nix shell)'
echo ''
bacon serve
