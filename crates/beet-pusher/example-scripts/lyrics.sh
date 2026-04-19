#!/usr/bin/env bash

cd $(dirname $0)

input=$1
shift

lyrics_re='^[0|1]$'
if ! [[ $input =~ $lyrics_re ]] ; then
  echo "USAGE: $0 0|1"
  echo ""
  echo "error: invalid has_lyrics value \"$input\""

  exit 1
fi
has_lyrics=$input

source ./get_current_id.sh

./run_beet_cmd.sh modify id:$current_id has_lyrics=$has_lyrics
