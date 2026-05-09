# Checks - available to use as a pre-commit hook
#
# WARNING: Does not separate staged vs unstaged changes!
# (only the staged files for certain checks)
#
# install via:
#    ln -s ../../pre-commit.sh .git/hooks/pre-commit

cd "$(git rev-parse --show-toplevel)"

# Run tests
true \
  && (echo "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986  COPYING" | sha256sum -c - --strict) \
  && cargo xtask checks --quiet $*
RESULT=$?

# TODO if nix is reinstated
# if [ $RESULT -eq 0 ]; then
#   nix --version >/dev/null 2>&1
#   if [ $? -eq 0 ]; then
#     nix flake check
#     RESULT=$?
#   else
#     echo "No nix found, skipping 'nix flake check'"
#   fi
# fi

# Exit with status from test-run: nonzero prevents commit
[ $RESULT -ne 0 ] && exit 1
exit 0
