## beet-pusher/template-scripts

Example usage of the `publish_id_file` configuration option.

### Entrypoints
1. `info.sh` - prints info for the current item
1. `grouping.sh` - sets the `grouping` key of the current item
1. `lyrics.sh` - sets the `has_lyrics` extended key of the current item

### Plumbing
1. `run_beet_cmd.sh` - allows for easily running `beet` on a different host (e.g. `ssh user@host beet`)
1. `get_current_id.sh` - reads the contents of `current_item_id.txt`, verifying it is numeric
