#!/usr/bin/env bash

cd $(dirname $0)

input=$1
shift

lyrics_re='^[1|2|3|4|5]$'
if ! [[ $input =~ $lyrics_re ]] ; then
  echo "USAGE: $0 1|2|3|4|5"
  echo ""
  echo "error: invalid grouping value \"$input\""

  exit 1
fi
grouping=$input

source ./get_current_id.sh

./run_beet_cmd.sh modify id:$current_id grouping=$grouping
