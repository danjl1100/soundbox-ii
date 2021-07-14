# Pre-Commit Hook
#
# install via:
#    ln -s ../../pre-commit.sh .git/hooks/pre-commit
#
STASH_NAME="pre-commit-$(date +%s)"
git stash save -q --keep-index "${STASH_NAME}"

cargo clippy --workspace \
  && cargo test --workspace \
  && cargo doc --workspace --no-deps -q \
  && echo "Outstanding cargo fmt files:" && cargo fmt --all -- --check -l
RESULT=$?

STASHES=$(git stash list | grep "${STASH_NAME}")
if [[ ${STASHES} != "" ]]; then
  git stash pop -q
fi
[ $RESULT -ne 0 ] && exit 1
exit 0
