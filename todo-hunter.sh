#! /usr/bin/env nix-shell
#! nix-shell -i bash -p gnugrep

# not-so-clever evasion tactic to exclude this script from the search results
NEEDLE="TO"
NEEDLE="${NEEDLE}DO"

find . -type f -not -path './.git/*' -not -path '*/target/*' -not -path './dist/*' | xargs grep -ni "${NEEDLE}" --color
COUNT=$(find . -type f -not -path './.git/*' -not -path '*/target/*' -not -path './dist/*' | xargs grep -ni "${NEEDLE}" | wc -l)
echo "Found ${COUNT} total TODOs"
