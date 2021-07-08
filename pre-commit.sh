# Pre-Commit Hook
#
# install via:
#    ln -s ../../pre-commit.sh .git/hooks/pre-commit
#
STASH_NAME="pre-commit-$(date +%s)"
git stash save -q --keep-index "${STASH_NAME}"

cargo test --workspace
RESULT=$?

STASHES=$(git stash list)
if [[ ${STASHES} == "${STASH_NAME}"]]; then
  git stash pop -q
fi
[ $RESULT -ne 0 ] && exit 1
exit 0
