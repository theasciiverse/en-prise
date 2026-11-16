# En Prise

Chess where the pieces fight, after Battle Chess (Interplay, 1988), for
the Asciiverse catalog. Rust + Ratatui. Ideation 2026-10-20 (David: an
action version of chess, rendered isometrically in the terminal, "no
grander plans than that"); built 2026-11-16 (David: "do you want to take
a stab at this one?").

## The name

"En prise" is the chess term for a piece left open to capture, the moment
this game is about; the game marks your own pieces that are en prise. Chosen
by David 2026-10-20 over Skirmate, Isomate, Pawnstorm and Rook & Ruin. A web
search that day found no video game of that name; there is a 2013 card
game, "En Prise Chess Cards" (not a trademark search). "Battle Chess" is
Interplay's title and is not used.

## Decided

- An action version of chess, an isometric 3D board in the terminal
- Our own pieces, characters and animations: nothing taken from Battle Chess
- 0.1.0 (2026-11-16) plays the Battle Chess way: chess decides the capture,
  the fight shows it

## Open

- An Archon-style mode (Free Fall Associates, 1983), where the two pieces
  really fight and the winner keeps the square, was saved for later by
  David (2026-10-20), maybe as a choice beside the shown fights. Not built.

## Status (0.1.0, 2026-11-16): playable

| File | What |
|---|---|
| `src/game.rs` | the game on cozy-chess (MIT): moves played and taken back, what a move did (castling is the king taking its rook in cozy-chess; en passant's victim), status (mate, stalemate, fifty moves, threefold, bare material), SAN, taken pieces, `en_prise` (attacked and undefended, or attacked by something cheaper) |
| `src/ai.rs` | alpha-beta, iterative deepening, capture search, killers, check extension, square tables; Easy (depth 2, noise 80), Normal (depth 5, 1.2 s), Hard (depth 12, 3.5 s) |
| `src/model.rs` | the six figures from lathe shapes and boxes, each holding something at a shoulder that swings (spear, crook, wand, sword); `Pose` (place, turn, lean, roll, lift, squash, arm, flash, size) and `Break` (shatter, dissolve, split) |
| `src/scene.rs` | the board (squares on a framed slab), lighting, the camera (yaw, pitch, zoom, target; nearly orthographic), effects (sparks, beams, stones), picking |
| `src/anim.rs` | a move played out: walks (a knight leaps), castling with the rook, promotion, and the six fights, each timed in seconds with its sounds; the camera closes in side-on for a fight |
| `src/app.rs` | modes (White, Black, two players, watch), the computer in a thread, picking up and putting down, promotion choice, take back, cursor that follows the turned board, the frame's figures and marks |
| `src/ui.rs` | the view, the side panel (from 76 columns), menu, help, promotion box |
| `src/raster.rs`, `src/audio.rs`, `src/rng.rs` | copied from Möbius Chess (the rasteriser from Ratty's Cube); duel sounds added |

The fights (fight time in seconds at full speed; quick is 1.8x, off skips
them and the taken piece shatters):

- Pawn: spear levelled, three jabs, the victim tips back and shatters
- Knight: rears, crashes down in a ring of dust, the victim flies and shatters where it lands
- Bishop: crook raised, a beam of light, the victim dissolves upward
- Rook: two stones lobbed from the battlements, the second breaks it
- Queen: a spin, a ring of light, the victim lifted and shattered
- King: sword raised, one blow, a line of light, the victim falls in two

```bash
cargo run --release
cargo run --release -- --shot out.ppm --fen FEN --move e4d5 --at 1.2 --size 120x36   # one frame as a picture
cargo test --release      # 21 tests
RUST_LOG=en_prise=debug cargo run --release 2>en-prise.log
```

`--shot` draws a frame without a terminal: a position, a move and how far
into it, as a PPM (text cells as blocks). That is how the fights were
checked by eye.

Session logging per the Asciiverse convention: every key and mouse event,
moves with what they took, state changes, a snapshot every 500 ms.

## Not done yet

- Not played by a person yet (tests, tmux and frames only)
- Played and recorded at a cell ratio of 1 (castrec cannot report pixels);
  real terminals report theirs and the board is drawn for it

## Release

- Tags `vX.Y.Z`; `.github/workflows/release.yml` from Möbius Chess (glibc
  Linux needing libasound2, macOS, Windows)
- Preview `asciiverse/en-prise.cast` by `asciiverse/record-preview.sh`:
  watch mode, Normal, seed 7, 45 s at 10 frames a second, 5.8 MB;
  thumbnail at 20 s (8. Nxc6, the knight's fight close up)
- Catalog drafts in `asciiverse/en-prise.toml` and `.md`, for `hub/catalog`
