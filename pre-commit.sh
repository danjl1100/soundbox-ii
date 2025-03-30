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

COPYRIGHT_TEXT="Copyright (C) 2021-$(date +%Y)  Daniel Lambert. Licensed under GPL-3.0-or-later"

# Run tests
true \
  && echo "Missing copyright notice in changed files:" \
    && [[ ! $( \
        cd "$(git rev-parse --show-toplevel)" && git diff --name-only HEAD | grep '.rs$' | \
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

if [ $RESULT -eq 0 ]; then
  nix --version >/dev/null 2>&1
  if [ $? -eq 0 ]; then
    nix flake check
    RESULT=$?
  else
    echo "No nix found, skipping 'nix flake check'"
  fi
fi

# Restore changes
# again, source: https://stackoverflow.com/a/20480591/5742216
git reset --hard -q && git stash apply --index -q && git stash drop -q

# Exit with status from test-run: nonzero prevents commit
[ $RESULT -ne 0 ] && exit 1
exit 0
