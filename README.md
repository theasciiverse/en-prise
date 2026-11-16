# En Prise

Chess where the pieces fight, on an isometric board in your terminal.

Every capture is played out on its square, and each kind of piece fights
its own way. The pawn jabs with its spear, the knight rears and crashes
down, the bishop calls down a bolt of light, the rook fires stones from its
battlements, the queen spins up a ring of magic, and the king brings his
sword down once. Chess has already decided who wins; the fight only shows
it. A piece of yours that can be taken for profit glows red: it is *en
prise*.

Needs a terminal with 24-bit colour; plays from 76x24 with the side panel,
smaller without it.

## Playing

- Click a piece and then where it goes, or move the cursor with the arrows
  (up is away from you) and press Enter
- **Space**: skip a fight; **d**: fights full, quick or off
- **u**: take back; **, .**: turn the board a quarter; drag to turn and tilt
- **p**: en prise marks on or off; **+ -**: zoom; **0**: look home
- **m**: sound; **Esc**: menu (play White or Black, two players, watch, the
  computer's strength, fights); **?**: help; **q**: quit

```bash
cargo run --release                  # you play White
cargo run --release -- --watch       # the computer against itself
cargo run --release -- --black --hard --quick
```

## How it is made

Rust and Ratatui; the board, the figures and the effects are triangles
drawn by a small software rasteriser into half-block cells (two pixels a
cell), with a depth buffer. The rules come from cozy-chess (MIT). The
computer player is our own alpha-beta search. The sounds are synthesised,
no sound files.

## Credits

In the spirit of Battle Chess (Interplay, 1988); our own pieces, fights and
code. The rasteriser comes from Ratty's Cube, by way of Möbius Chess. MIT.
