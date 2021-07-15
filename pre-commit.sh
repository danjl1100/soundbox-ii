# Pre-Commit Hook
#
# install via:
#    ln -s ../../pre-commit.sh .git/hooks/pre-commit

# First, stash index and workdir, keeping only the
# to-be-committed changes in the working directory.
# source: https://stackoverflow.com/a/20480591/5742216
old_stash=$(git rev-parse -q --verify refs/stash)
git stash save -q --keep-index "pre-commit-$(date +%s)"
new_stash=$(git rev-parse -q --verify refs/stash)
# If there were no changes (e.g. '--amend' or '--allow-empty')
# then nothing was stashed, and we should skip everything,
# including the tests themselves. (Presumably the tests passed
# on the previous commit, so there is no need to re-run them.)
if [ "${old_stash}" = "${new_stash}" ]; then
  echo "pre-commit script: no changes to test"
  exit 0
fi

# Run tests
true \
  && echo "Outstanding cargo fmt files:" && cargo fmt --all -- --check -l && echo "[none]" \
  && cargo clippy --workspace \
  && cargo test --workspace \
  && cargo doc --workspace --no-deps -q \
  && true # trailing commas for the win
RESULT=$?

# Restore changes
# again, source: https://stackoverflow.com/a/20480591/5742216
git reset --hard -q && git stash apply --index -q && git stash drop -q

# Exit with status from test-run: nonzero prevents commit
[ $RESULT -ne 0 ] && exit 1
exit 0
