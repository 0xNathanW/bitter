# Notes

IMMEDIATE TASK:
    - Get write tests working.
    - Send tracker info.
    - Setup seeder/leecher tests.
    - on pause, disconnect from peers, however, cache information eg. bitfield, before doing so.
    - Need to iter over torrents/peer row data.
      - Only getting first row atm.
    - auto disconnect from seeds when downloaded.
    - Show metainfo on the file-explorer before loading in.
    - add timeout for binding listener port.
    - checking existing is off by one piece, sometimes panics.

TODO:
    - Actual algorithm for piece selection.
    - Designation of peers based on whether they are seeders.
    - Move tracker to a seperate thread/task.
    - Implement upd/wss trackers.
    - Request queuing, ie, waiting for buffer to fill before sending multiple requests in a batch.
    - can i replace Option<Joinhandle> with just Joinhandle, see: https://github.com/ratatui-org/async-template/blob/main/ratatui-counter/src/tui.rs
    - tui configurable:
      - foreground colour.
      - background colour.
      - download units.
    - When checking existing pieces, make less syscalls because we are making sequential reads.
        - Decide how much to read at a time.
        - Think about the disk tasks.
    - Implement on_parole for peers.
      - If they fail a hashcheck, only download whole pieces from them until that pieces proves they do/don't send bad data.

PICKER:
    - if can pick from partial pieces:
      - pick FREE blocks sequentially from partial pieces < target
    - while < target
      - if can pick new piece:
        - pick FREE blocks sequentially from partial pieces < target
        - add new pieces to partial pieces
      - else set END_GAME:
        - pick REQUESTED BLOCKS sequentially from partial pieces < target
          - ensure blocks not already in peer's own request queue 

OUTLINE:

disk:
    - handles read/writes 


tracker control flow:
    1. wait for change in watch chan:
    2. recv value.
    if announce:
        
    else: 
        continue, the torrent needs no peers.


