# Notes

IMMEDIATE TASK:
    - working on piece_write
    - test multifile.
      - testing by picking pieces in reverse, because last files are smaller.
      - failing on trackers.
        - actually works now but i think **left** is still wrong.
    - are we checking peers actually have the piece we are sending requests for?
      - prevents duplicates

TODO:
    - Implement block reads!
    - Setup seeder/leecher tests.
    - Manager for multiple torrents at once.
      - Setup TorrentManager struct.
      - Move disk outside of torrent struct.
    - Actual algorithm for piece selection.
    - Designation of peers based on whether they are seeders.
    - Move tracker to a seperate thread/task.
    - Don't expose metainfo to external api.
    - Implement upd/wss trackers.
    - Request queuing, ie, waiting for buffer to fill before sending multiple requests in a batch.


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


