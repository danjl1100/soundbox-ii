#!/usr/bin/env bash

cd $(dirname $0)

source ./get_current_id.sh

./run_beet_cmd.sh "ls id:$current_id -f '[id=\$id lyrics=\$has_lyrics grouping=\$grouping] \$artist - \$album - \$title'"
