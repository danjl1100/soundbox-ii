untrusted_value=$(cat ../current_item_id.txt)

number_re='^[0-9]+$'
if ! [[ $untrusted_value =~ $number_re ]] ; then
  echo "error: invalid id in ../current_item_id.txt"
  exit 1
fi

current_id=$untrusted_value
