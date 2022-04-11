# soundbox-ii
<!-- [firefly-iii](https://github.com/firefly-iii/firefly-iii) is cool enough to put iteration numbers in the title, so why not? -->

*don't keep your sounds boxed up*


(Work-in-progress) VLC music controller and track sequencer to navigate a massive music library (best paired with a curated [beets](https://github.com/beetbox/beets) music library).



## Roadmap - Scope and Features

- [x] **Gate 1 - Basic VLC Playback Controls**
First gate is basic VLC playback control on desktop and mobile clients. At this stage, VLC still loads the `.m3u` file as directed. VLC fully controls playlist item progression.

1. Basic VLC playback controls
	- play, pause, stop
	- next, previous
	- seek
	- album art
	- volume
	- repeat, shuffle modes

Remaining down-in-the-trenches tasks:
- [x] Reload delay display on frontend
- [x] Encapsulate pieces of entry points
    - [x] backend `launch(..)` entry fn
    - [x] frontend `new(..)` fn
- [x] Album artwork passthrough HTTP response
- [x] Mobile-first Frontend layout design (Gate-1 specific controls **only**)
- [x] Logic layer on top of vlc-controller
    - [x] After skip next, re-poll until track info changes (max `N` repolls)
    - [x] move polling logic into vlc-http (every few seconds status, playlist)
    - [x] Rate-limit access to VLC client, in case of malicious clients

_____

- [ ] **Gate  2 - Rust Playlist Control**
Second gate is Rust taking control of playlist, spoon-feeding VLC one "on-deck" item as it plays its single item. VLC will only have 1-2 items in its play queue at any given time.
	- [ ] Bonus task: "Remove" action for the on-deck item, promoting the next item.

2. Rust Queue managment (basic)
	- **source** is single playlist text file, or folder/glob pattern.
	- basic repeat / shuffle modes
	- 10 up-next items listed, for review/removal

- [ ] **Gate 3 - Rust source-filters and merges**
Third gate is adding complex queries to the source selection, and additional sources (beets cli filters, e.g. added:2021.. grouping:5), with ratio-merging of different source selections.
	- [ ] Bonus task: 

3. Rust Queue management (stretch) - source-only filtering tree
	- **source-filters** select a subset of the beets library (*to be split into separate source / filter blocks in next phase*)
	- **merges** join multiple sources (discrete 1:3, or randomized 25%:75%)
		- Front-end can implement "manual switch" type as a "0:1 ot 1:0" merge, 1:0 for choosing between two sources.
	- inspect state of each node in the graph (5-10 most recent accepted items)

- [ ] **Gate 4 - Rust downstream filters**
Fourth gate is adding downstream filters in the queue graph.

4. Rust Queue management (stretch) - full tree
	- **source** is beets library (everything)
	- **filters** block items from progressing
		- Back-end translates the graph into an equivalent "top-level filtering only" graph from previous phase.
		- *Conceptually, downstream filters just send a signal to top-level sources filters to add to their filter list.  Same end result as only allowing top-level source filters, but easier to reason about for users.*
	- **manual/tap filters** queue N items, user can reject any item in the queue
	- **merges** join multiple sources (discrete 1:3, or randomized 25%:75%)
		- Front-end can implement "manual switch" type as a "0:1 ot 1:0" merge, 1:0 for choosing between two sources.
	- Inspect state of each node in the graph (5-10 most recent accepted items)

- Ex: Manual switch added to the bottom of the graph for "lyrics / no-lyrics" is an easy way to update dozens of top-level source filters, merged through a series of complex tunable ratio combinations.


## Implementation
Three modules facilitate connections to the back end services:
1. VLC Interface (vlc-http)
	- glue for HTTP commands
	- provides HTTP status/playlist
2. Beets Command (beet-cli)
	- forward advanced queries to beets command 
	- (stretch) modify HasLyrics and Rating fields 
3. Beets Info (beet-http)
	- glue for beets HTTP endpoint `/item/[ID],[ID],...` to get detailed meta info


Additional modules provide logic.
- vlc_play_manager
	- *[Gate 2] accepts a "item provider" (iterator) for items to feed to VLC*
- plugboard
	- manages source, filter, merge nodes and their connections

Unified front-end endpoints map
| Description | Provider | Rust URL (POST) | Comments |
|-------------|----------|-----------------|----------|
| Play | vlc-http | /v1/play
| Pause | vlc-http | /v1/pause
| Stop | vlc-http* | /v1/stop | ***provider** changes in phase 2* |
| Next | vlc-http* | /v1/next | ***provider** changes in phase 2* |
| Previous | vlc-http* | /v1/previous | ***provider** changes in phase 2* |
| Seek To | vlc-http | /v1/seek_to {`seconds`}
| Seek | vlc-http | /v1/seek {`delta`}
| Album Art | vlc-http | /v1/art
| Volume | vlc-http | /v1/volume {`percent`}
| Set Repeat | vlc-http<sup>X</sup> | /v1/repeat {`enable`?} | ***deprecated** in phase 2* |
| Set Shuffle | vlc-http<sup>X</sup> | /v1/shuffle {`enable`?} | ***deprecated** in phase 2* |
|||| TODO in Phase 2:
| List Queue (source-id) | | | for phase 2, source-id=0 is the only queue
| Set Filter (source-id, filter) | | | for phase 2, source-id=0 filter is jerry-rigged to be playlist name
| Remove Queue (source-id, idx) | | | index-only, since item-id can appear multiple places within a queue
|||| TODO in Phase 3:
| Set Source (source-id, sink-id) | | | merges will have multiple sink-ids and one source-id
| Add Merge
| Set Merge (N, [ratio; N])
|||| TODO in Phase 4:
| Add Source
| Add Filter (filter)

Source/Sink graph details, by phase
1. Phase 1: no source/sink graph
2. Phase 2: single root source-id=0 is a merge with 1 input as "all songs".   filter is jerry-rigged to be the selected playlist name
3. Phase 3: root merge source-id=0 is a (hidden) merge with N sinks, hardcoded filter=None. User-visible output is actually sink-id=0.0, all not-connected sources are intrinsically linked to (hidden merge) sink-ids=0.1, 0.2, .... with hard-coded merge frequency of 0.
    - Allows Rust ownership pattern of root merge source-id=0 owning all nodes.

Illustration
```text
Phase 1:  N/A

Phase 2:

  0:[ filter=4+stars.m3u ]

Phase 3:

  1:[| filter='rating::"4|5"' ]   2:[| filter='artist::"Taylor Swift"']
     |                               |
     V sink 0.0                      V sink 0.1
  0:[ filter N/A                                ]
```




___

### API Details - VLC Interface

| Module: | vlc-http |
|---------|----------|
| Source: | [/share/doc/vlc/lua/http/requests/README.txt](file:///nix/store/n6zp4qmfv6s0mj31abrd5w9cfwjqxc07-vlc-3.0.12/share/doc/vlc/lua/http/requests/README.txt)|
| Params: | `HOSTNAME`:`PORT` |

- VLC Definitions
	- **VLC Now Item** - item currently playing/paused (none if stopped)
	- **VLC Playlist**- items known to VLC in the current queue (includes previous, current, and next items)

VLC HTTP - Method is always `GET`
<!--                               Rust URL |   -->
| Description | VLC URL | Rust fn | 
|-------------|---------|---------|
| Now - Album Art | /art | -> get_art() |
| Now - Info/Status | /requests/status.json | -> get_now_status() |
| Now - Play from URI | /requests/status.json?command=**in_play**&input=`URI` | <- set_play_uri(`URI`) |
| Now - Play Playlist Item | /requests/status.json?command=**pl_play**&id=`VLC_ID` | <- set_play_playlist_item(`VLC_ID`) |
| Now - Resume | /requests/status.json?command=**pl_forceresume** | <- set_resume() |
| Now - Pause | /requests/status.json?command=**pl_forcepause** | <- set_pause() |
| Now - Stop | /requests/status.json?command=**pl_stop** | <- set_stop() |
| Now - Next | /requests/status.json?command=**pl_next** | <- set_next() |
| Now - Previous | /requests/status.json?command=**pl_previous** | <- set_previous() |
| Now - Speed | /requests/status.json?command=**rate**&val=`RATE` | <- set_rate(`RATE`) |
| Now - Volume | /requests/status.json?command=**volume**&val=`VOL` | <- set_volume(`VOL`) |
| Now - Seek | /requests/status.json?command=**seek**&val=`SECONDS` | <- set_seek_seconds(`SECONDS`) |
||
| Playlist - Album Art | /art?item=`VLC_ID` | -> get_art_for_id(`VLC_ID`) |
| Playlist - Add Item | /requests/status.json?command=**in_enqueue**&input=`URI` | <- set_add_uri(`URI`) |
| Playlist - Clear | /requests/status.json?command=**pl_empty** | <- set_clear_playlist() |
| Playlist - Toggle Random | /requests/status.json?command=**pl_random** | <- set_toggle_random() |
| Playlist - Toggle Loop | /requests/status.json?command=**pl_loop** | <- set_toggle_loop() |
| Playlist - Toggle Repeat | /requests/status.json?command=**pl_repeat** | <- set_toggle_repeat() |
| Playlist - Info | /requests/playlist.json | -> get_playlist() |

- Composite endpoints:
	- `set_playlist_mode(PlaylistMode)` Monitor `get_status()` and run `set_toggle_*()` to sync to known state:
		- `struct PlaylistMode { shuffle: bool, repeat: Repeat }`
		- `enum Repeat { Off, All, Single }`


___

### API Details - Beets CLI

| Module: | beets-cli |
|---------|-----------|
| Source: | [beets.readthedocs.io - Command Line Interface](https://beets.readthedocs.io/en/stable/reference/cli.html) |
| Params: | Path to `beet` command (default `beet`) |

- Definitions
	- Beets `ID` is authoritative, with global scope.
	- Rating is 1, 2, 3, 4, or 5.

| Description | Beets CLI | Rust fn | Rust URL |
|-------------|-----------|---------|----------|
| Search Items | beet list `QUERY...` -f '\$id\|\$path' | -> get_items(`QUERY...`) |
| Edit Item Rating | beet modify id:`ID` grouping=`RATING` | <- set_rating(`ID`, `RATING`) | POST /v1/beet/`ID`/set_meta { rating=`RATING` }
| Edit Item Has-Lyrics | beet modify id:`ID` has_lyrics=`0|1` | <- set_has_lyrics(`ID`, bool) | POST /v1/beet/`ID`/set_meta { has_lyrics=`0|1` }


___

### API Details - Beets HTTP Forwarder

| Module: | beets-http |
|---------|------------|
| Source: | [beets.readthedocs.io - Web Plugin - JSON API](https://beets.readthedocs.io/en/stable/plugins/web.html#json-api) |
| Params: | `HOSTNAME`:`PORT` |

**Note:** Only consumer is direct from JS (no Rust code interaction).

See inspiration for HTTP forwarder in [Actix examples - http-proxy](https://github.com/actix/examples/tree/master/basics/http-proxy).

| Description | Beets HTTP | Rust URL | 
|-------------|------------|----------|
| Items Meta Info | /item/`ID...` | /v1/beets-info/`ID...` |

- Response fields:
	- title
	- album
	- artist
	- length
	- has_lyrics
	- grouping (my alias for "rating")
	- year
	- added (unix timestamp)
	- track, tracktotal
	- mb_trackid, mb_releasegroupid, mb_albumid, mb_artistid
