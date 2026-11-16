# En Prise

**Chess, with the gloves off.**

You know the moment. You slide your bishop across the board, the enemy
knight is standing right there, and in ordinary chess you lift it off with
a polite little click. In En Prise you sit back and watch your bishop raise
his crook, call down a column of light, and melt that knight into the air
while the camera swings round to give you the best seat in the house.

Every capture is a fight, and every piece has its own way of winning one:

- **The pawn** levels its spear and jabs, once, twice, and on the third the
  enemy topples over and shatters
- **The knight** rears up on its hind legs and comes down in a ring of
  dust, and its victim goes flying
- **The bishop** brings down the light, and the enemy dissolves upward
- **The rook** lobs stones from its battlements; the first is a warning,
  the second is not
- **The queen** spins on the spot, throws out a ring of light, lifts her
  prey off the board and breaks it
- **The king** raises his sword and brings it down once. Once is enough:
  a line of light, and his enemy falls in two

Chess has already decided who wins. The fight is the victory lap.

And because every piece you leave hanging is about to meet one of the
above, En Prise watches your back: any piece of yours that can be taken
for profit glows red. It is *en prise*. Do something about it.

## The game underneath

Real chess, all of it: castling (the rook marches round), en passant,
promotion (the pawn shrinks into light and something grander grows out
of it), check, mate, stalemate, threefold repetition and the fifty-move
rule. Play White or Black against a computer that is gentle on Easy,
honest on Normal and thinks hard on Hard; play a friend at one keyboard;
or put the kettle on and let the computer fight itself.

The board is a slab of 3D in your terminal, seen from the corner like an
isometric game of the late eighties. Turn it, tilt it, zoom in on the
carnage.

## Playing

```bash
cargo run --release                  # you play White
cargo run --release -- --watch       # the computer against itself
cargo run --release -- --black --hard --quick
```

- Click a piece and then where it goes, or move the cursor with the arrows
  (up is away from you) and press **Enter**
- **Space**: skip a fight; **d**: fights full, quick or off
- **u**: take back; **, .**: turn the board a quarter; drag to turn and tilt
- **p**: en prise marks on or off; **+ -**: zoom; **0**: look home
- **m**: sound; **Esc**: menu (sides, two players, watch, strength,
  fights); **?**: help; **q**: quit

Wants a terminal with 24-bit colour. From 76 columns the move list, the
taken pieces and your hanging ones sit in a panel beside the board.

## How it is made

Every figure is turned like a real chessman: lathe shapes and blocks,
triangles drawn by a small software rasteriser two pixels to a character
cell, with a depth buffer so the knight can rear up in front of the rook
behind it. The sparks, beams and stones are triangles too. The sounds are
synthesised on the fly, so there is not a single sound file in the game.
The rules come from cozy-chess (MIT); the computer player is our own
alpha-beta search. Rust and Ratatui.

## Credits

In the spirit of Battle Chess (Interplay, 1988), the game that first made
chessmen fight; our own pieces, fights and code. The rasteriser comes from
Ratty's Cube, by way of Möbius Chess. Chess rules by cozy-chess
(analog-hors, MIT). En Prise is MIT.
