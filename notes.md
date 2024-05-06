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



    FATAL - This doesn't exist in Rust, because you panic!(), but I might as well include it either way.
    You should use FATAL to log errors that are about to crash your application.
    Example: FATAL: Syntax error in configuration file. Aborting.
    ERROR - You should use ERROR to log errors within some specific task. These errors are causing a task to fail, but it's not the end of the world.
    Example: ERROR: Broken pipe responding to request
    WARN - You should use WARN to log errors that were recovered from. For example, things you're retrying again (if this fails again and you give up, it should end up being an ERROR)
    INFO - You should use INFO to log informational messages like major status updates that are important at runtime:
        INFO: Server listening on port 80
        INFO: Logged into <API> as <USER>.
        INFO: Completed daily database expiration task.
    DEBUG - Now to the important part. DEBUG logs should basically always only log variables or decisions. You could use DEBUG logs whenever there's variables you need to log, or after a major decision like "user did this" or "choosing to use chunked sending"
    TRACE - TRACE logs are purely "I am here!" logs. They are NOT used at the beginning or end of a block like an if statement to indicate "Hey, I made this choice!", they indicate "Hey, I'm making an API request... I'm done!"
as