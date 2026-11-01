# Keys

This page lists the keys the editor answers to, grouped by where they work. It is taken from the binding table in `crates/bevy_grimoire_mapper/src/keys.rs`. Hold `K` in the editor to see the same keys in an overlay.

## How to read this page

Cmd means the Command key on a Mac. Shift and Alt keep their names. Alt is Option on a Mac. A row with two chords, such as `Cmd+Z` and `Cmd+Shift+Z`, is two bindings that do opposite jobs.

The editor has three doors. A door is one of its three workspaces:

- **Map** builds a level out of pieces.
- **Kit** makes the reusable content: meshes, tiles, and compositions.
- **Rigs** checks the animation rigs.

A piece is one placed item on a map. A composition is a named group of pieces that you place as one. A kit is the set of meshes, tiles, and compositions you make once and reuse.

The "Works when" column names the state the editor must be in:

- **Any time** means the key works in every state of that tab.
- **Nothing in hand** means no piece is held.
- **While a piece is in hand** means you have picked a piece up and not yet dropped it.
- **While a derivation waits** means the editor has proposed a result and is waiting for a yes or no.
- **While browsing tiles** means the Tiles panel is live, and the arrow keys walk the tiles.
- **While a composition is being picked** means the composition picker has the arrow keys.

A key marked "in filter boxes" still walks its list while a filter box has focus. Every other key pauses while you type in a name, an id, or a filter box. Typing the letter `S` in a name does not save the map, because saving is `Cmd+S`.

## Every key is a command

Each key on this page is also a command with an id that does not change, such as `file.save`, `edit.undo` or `tiles.align_forward`. The part before the dot is the category a menu groups it under. A menu, the command palette and an agent name a command by its id, so renaming a key never breaks them. `crates/bevy_grimoire_mapper/src/commands.rs` lists every id and its title, and a test fails when two commands share an id, a title, or a shortcut on a tab where both work.

## Everywhere

These keys work on every tab.

| Key | Works when | Does |
| --- | --- | --- |
| `1`, `2`, `3` | Any time | Go to panel 1, 2, or 3 of the door you are in. |
| `=` | Any time | Switch between the Map and Kit doors. |
| `Cmd+Delete` | Any time | Define this piece. On a Mac, the Delete key sends Backspace. |
| `K` (held) | Any time | Show every key and what it does. Release to hide it. |
| `Cmd+S` | Any time | Save. |
| `Cmd+\` | Any time | Snip an area. |
| `Cmd+O` | Any time | Go back to the menu. |
| `Cmd+E` | Any time | Show every error this session. |
| `Cmd+C` | Any time | Copy this tab's text. |
| `Cmd+F` | Any time | Filter the list. |
| `Esc` | Any time | Back out of the current state. |
| `W`, `A`, `S`, `D` | Any time | Pan forward, left, back, and right. |
| `Q`, `E` | Any time | Turn the view left and right. |

## Map

These keys work on the Map tab.

| Key | Works when | Does |
| --- | --- | --- |
| `Cmd+Z` | Any time | Undo. |
| `Cmd+Shift+Z` | Any time | Redo. |
| `H` | Any time | Target the stack. |
| `Up` / `Down` | Nothing in hand | Walk the palette. Hold Shift to move five at a time. Works in filter boxes. |
| `P` | Nothing in hand | Take the arrow keys to the compositions. |
| `Up` / `Down` | While a composition is being picked | Walk the compositions. Hold Shift to move five at a time. |
| `Enter` | While a composition is being picked | Arm this composition for the map. |
| `R` | Any time | Turn left, 15° a press: the armed piece if one is armed, otherwise the placed piece under the pointer. |
| `T` | Any time | Turn right, 15° a press: the armed piece if one is armed, otherwise the placed piece under the pointer. |
| `Y` | Any time | Tip about x, a quarter turn: the armed piece if one is armed, otherwise the piece under the pointer. |
| `U` | Any time | Tip about z, a quarter turn: the armed piece if one is armed, otherwise the piece under the pointer. |
| `V` | Any time | Straighten the piece. |
| `J` | Any time | Change the grid rung: tile, fine, then finer. |
| `[` / `]` | Any time | Lower / lift this piece. |
| `X` | Any time | Enter removal mode. |
| `Home` | Any time | Frame the view: the locked placement when there is one, otherwise the whole map. |
| `B` | Any time | Move. |
| `Shift+B` | Any time | Clone. |
| `Cmd+B` | Any time | Run. |
| `M` | Any time | Keep the selected set as a composition. |
| `N` | Any time | Rename the map. |
| `O` | Any time | Pin or unpin. |
| `F` | Nothing in hand | Fill with the brush. |
| `G` | Nothing in hand | Fill with the learned source. |
| `Shift+G` | Nothing in hand | Fill with the declared source. |
| `Cmd+G` | Nothing in hand | Fill with the composed source. |
| `Enter` | While a derivation waits | Keep this layout. |
| `Esc` | While a derivation waits | Throw the layout away. |

## Meshes

These keys work on the Meshes panel of the Kit door.

| Key | Works when | Does |
| --- | --- | --- |
| `Up` / `Down` | Any time | Walk the lists. Hold Shift to move five at a time. Works in filter boxes. |
| `Left` | Any time | Move focus to the candidates. Works in filter boxes. |
| `Right` | Any time | Move focus to the library. Works in filter boxes. |
| `I` | Any time | Type an id. |
| `M` | Any time | Mount. |
| `Enter` | Nothing in hand | Add or update this tile. |
| `Space` | No piece in hand | Open or close the pack. |
| `Enter` | While a derivation waits | Keep the derived edges. |
| `Esc` | While a derivation waits | Throw the derived edges away. |
| `R` | Any time | Rescan. |
| `Shift+R` | Any time | Exclude the pack. |
| `Delete` | Any time | Send this back to NOT IMPORTED. |
| `Cmd+Z` | Any time | Undo this tab's last change. |
| `Cmd+Shift+Z` | Any time | Redo this tab's last change. |
| `T` | Any time | Move the cell cursor forward. |
| `F` | Any time | Move the cell cursor left. |
| `G` | Any time | Move the cell cursor back. |
| `H` | Any time | Move the cell cursor right. |
| `Shift+T` | Any time | Add a vocabulary token. |
| `[` / `]` | Any time | Previous / next layer. |
| `Z` | Any time | Mark this cell solid. |
| `X` | Any time | Mark this cell as an edge. |
| `V` | Any time | Clear this cell. |
| `B` | Any time | Rescan the solid. |
| `N`, `O`, `P` | Any time | Turn the mesh about x, about y, and about z. |
| `L` | Any time | Suggest labels. |
| `Shift+L` | Any time | Suggest labels for all. |
| `Shift+Y` | Any time | Abandon all suggestions. |
| `/` | Any time | Filter the tags. |

## Tiles

These keys work on the Tiles panel of the Kit door.

| Key | Works when | Does |
| --- | --- | --- |
| `Up` / `Down` | While browsing tiles | Walk the tiles. Hold Shift to move five at a time. Works in filter boxes. |
| `Right` | While browsing tiles | Go to the meshes. |
| `Left` | Nothing in hand | Go back to the tiles. |
| `Enter` | While browsing tiles | Open this tile. |
| `Up` / `Down` | Nothing in hand | Walk the library. Hold Shift to move five at a time. Works in filter boxes. |
| `Up` / `Down` | While a piece is in hand | Move the piece forward and back. |
| `Left` / `Right` | While a piece is in hand | Move the piece left and right. |
| `,` / `.` | While a piece is in hand | Step to the previous / next member. |
| `Shift` + arrow | While a piece is in hand | Flush the piece to that side. |
| `Space` | Not browsing tiles | Take the piece. `Esc` puts it back. |
| `[` / `]` | Not browsing tiles | Move the layer down / up. |
| `J` | Not browsing tiles | Make the grid deeper by thirds, wrapping around. |
| `Enter` | Nothing in hand | Drop the piece. |
| `Shift+Enter` | Nothing in hand | Drop the piece into a slot. |
| `R` | Not browsing tiles | Turn this piece. |
| `Delete` | Not browsing tiles | Remove this piece from the tile. |
| `Shift+Delete` | Not browsing tiles | Empty the tile. |
| `N` | Any time | Name a new tile. |
| `C` | Any time | Close the tile. |
| `Cmd+Z` | Not browsing tiles | Undo this tab's last change. |
| `Cmd+Shift+Z` | Not browsing tiles | Redo this tab's last change. |

## Anim

These keys work on the Rigs door's Anim tab.

| Key | Works when | Does |
| --- | --- | --- |
| `Up` / `Down` | Any time | Walk the rigs. Hold Shift to move five at a time. Works in filter boxes. |
| `Enter` | Any time | Adopt the measured values into the rig file. |
| `Cmd+Z` | Any time | Undo the last write. |
| `Cmd+Shift+Z` | Any time | Redo the last write. |
| `Left` / `Right` | Any time | Scrub the phase. Hold Shift to scrub finely. |
| `Space` | Any time | Play or scrub. |
| `C` | Any time | Check all rigs. |
| `G` | Any time | Toggle the ghost: measured over declared. |
| `V` | Any time | Change the view: figure, feet, side, or ground. |

## Compose

These keys work on the Compose tab of the Kit door.

| Key | Works when | Does |
| --- | --- | --- |
| `Up` / `Down` | Any time | Walk the focused list. Hold Shift to move five at a time. |
| `Enter` | Any time | Arm this composition for the map. |
| `I` | Any time | Rename this composition. |
| `Left` / `Right` | Any time | Choose which list the arrow keys walk. |
| `O` / `P` | Any time | Previous / next composition. |

## Text fields and menus

While a name, an id, or a filter box has focus, the editor reads your keys as text. Every shortcut above pauses, except the list keys marked "Works in filter boxes".
