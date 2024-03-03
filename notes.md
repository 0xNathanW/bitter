# Notes

IMMEDIATE TASK:
    - Implement seeding/continued downloading.
      - Check output_dir location for files.
        - Check hashes of already downloaded pieces. 
TODO:
    - Setup seeder/leecher tests.
    - Actual algorithm for piece selection.
    - Designation of peers based on whether they are seeders.
    - Move tracker to a seperate thread/task.
    - Don't expose metainfo to external api.
    - Implement upd/wss trackers.
    - Request queuing, ie, waiting for buffer to fill before sending multiple requests in a batch.
    - can i replace Option<Joinhandle> with just Joinhandle, see: https://github.com/ratatui-org/async-template/blob/main/ratatui-counter/src/tui.rs

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