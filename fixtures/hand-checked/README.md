# Hand-checked G-code fixtures

For steps that make G-code EasyGCode has no counterpart for, CLAUDE.md asks for a hand-checked
fixture instead of an EasyGCode one. `scripts/hand-checked-fixtures.py` works these out from
the geometry itself:
- in exact arithmetic (fractions, and 50-digit decimals for π and square roots);
- independently of EasyCAD's Rust code;
- with every move annotated, so a person can check any line with a calculator.

The golden tests (`crates/core/src/golden_tests.rs`) compare EasyCAD's G-code with them command
by command.

| Fixture | What it covers |
|---|---|
| `spiralize-tilted.gcode` | `spiralize` through tilted layers. A diamond (corners on the axes, 10 mm out) has its top cut by the plane z = 2 + 0.1·x, with three layers tweened in between. It prints at 5 mm³/s on the pellet printer, with no start/end blocks. Each move is extruded for its own layer height, and its speed follows from the flow. |

Regenerate them, or check that they're up to date (CI does):

```sh
python3 scripts/hand-checked-fixtures.py
python3 scripts/hand-checked-fixtures.py --check
```

## A move checked by hand

Take this move from `spiralize-tilted.gcode`, in turn 2 from the +X corner to the +Y corner:

```
G1 X125.000 Y135.000 Z1.125 E7.65601
```

- **Where:** the plane is 2 + 0.1·10 = 3 mm high at +X and 2 mm high at +Y.
  - Four layers stack up to it, so a layer is 0.75 mm thick at +X and 0.5 mm thick at +Y.
  - Turn 2 starts at +X at 2 × 0.75 = 1.5 mm.
  - A quarter turn later it is at +Y, at 2.25 × 0.5 = 1.125 mm.
  - With the bed centre at (125, 125), that gives X125 Y135 Z1.125.
- **Layer height:** the gap under the move runs from 0.75 to 0.5 mm. Its mean is 0.625 mm.
- **Length:** √(10² + 10² + 0.375²) = √200.140625 = 14.147107 mm.
- **Bead area:** (1 − 0.625) · 0.625 + π · 0.3125² = 0.234375 + 0.306796 = 0.541171 mm².
- **E:** 0.541171 × 14.147107 = 7.65601 mm³. The pellet printer's E is volumetric, with
  multiplier 1.
- **F:** 5 mm³/s × 60 / 0.541171 mm² = 554.35, which rounds to 554 mm/min (9.2 mm/s). The
  F554 was set on an earlier move with the same area, so this line doesn't repeat it.
  - On the thin side (−X), the moves have a 0.375 mm mean layer and a bead area of 0.344822 mm².
    They get F870 (14.5 mm/s): the same 5 mm³/s.
