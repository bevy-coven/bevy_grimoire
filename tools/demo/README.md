# Demo tooling

Dev-only. These scripts make the GIFs in `docs/media/`. They are not part of any crate and are not shipped: they drive a live editor, need Python 3, Pillow and ffmpeg on the machine that records, and are not needed to build or use bevy_emerge.

## What is here

- `record.py` runs a storyboard against a running editor. It sends the person's keys, clicks and drags over `bevy_debugger/input`, sends agent operations through the MCP server, and captures frames with the editor's own `emerge_capture_sequence`. It writes `<project>/target/loop/demo/<name>/events.json`.
- `render.py` turns that timeline into a GIF, with captions, keycaps and a cursor. It can also write a contact sheet and full-size stills.
- `storyboards/*.json` are the five demos: `hero`, `place_and_undo`, `agent_removes_then_undo`, `frame_the_map` and `move_the_camera`.

## Run

Start the editor on the project first, from its own shell. `record.py` never launches or closes the editor.

```sh
cd /path/to/project
BEVY_BRP_PORT=15710 /path/to/bevy_emerge_mapper . town > editor.log 2>&1
```

Record a storyboard against it:

```sh
BEVY_EMERGE_MCP=/path/to/bevy_emerge_mcp \
  python3 tools/demo/record.py tools/demo/storyboards/hero.json --port 15710 --project /path/to/project
```

Render the GIF and a contact sheet:

```sh
python3 tools/demo/render.py /path/to/project/target/loop/demo/hero/events.json \
  --out docs/media/hero.gif --width 960 --sheet /tmp/hero_sheet.png --stills 2.0,5.0,9.8
```

## Notes

- Capture rate: the storyboard's `fps` is sent to `emerge_capture_sequence`, which accepts 2 to 15 fps. The storyboards use 15.
- An agent operation can only start a sequence (frame 0 is the state before it), so each operation is its own chunk. The render holds one frame across the gap between chunks.
- Storyboard format is in the header of `record.py`.
