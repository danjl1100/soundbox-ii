# Checks - available to use as a pre-commit hook
#
# WARNING: Does not separate staged vs unstaged changes!
# (only the staged files for certain checks)
#
# install via:
#    ln -s ../../pre-commit.sh .git/hooks/pre-commit

COPYRIGHT_TEXT="Copyright (C) 2021-$(date +%Y)  Daniel Lambert. Licensed under GPL-3.0-or-later"

cd "$(git rev-parse --show-toplevel)"

# Run tests
true \
  && echo "Missing copyright notice in changed files:" \
    && [[ ! $( \
        git diff --cached --name-only HEAD | grep '.rs$' | \
        xargs --no-run-if-empty grep -LH "${COPYRIGHT_TEXT}" | tee /dev/stderr \
      ) ]] \
      || (echo "fix using:   echo \"// ${COPYRIGHT_TEXT}, see /COPYING file for details
\$(cat \$FILE)\" > \$FILE" && false) \
    && echo "[none]" \
  && (echo "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986  COPYING" | sha256sum -c - --strict) \
  && echo "Outstanding cargo fmt files:" && cargo fmt --all -- --check -l && echo "[none]" \
  && cargo clippy --workspace --all-targets --color always \
  && cargo test --workspace --color always \
  && cargo doc --workspace --no-deps -q --color always \
  && true # trailing commas for the win
RESULT=$?

# TODO when nix reinstated
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
