#!/usr/bin/env python3
"""Writes the hand-checked G-code fixtures in fixtures/hand-checked/.

Some steps make G-code that EasyGCode has no counterpart for (vase mode through tilted layers).
CLAUDE.md asks for a hand-checked fixture then: this script works the expected G-code out from
the geometry itself, in exact arithmetic (fractions, and decimals to 50 digits for pi and square
roots), independently of EasyCAD's Rust code, and annotates every move with the numbers a person
can check with a calculator. EasyCAD's golden tests compare their G-code with it
(crates/core/src/golden_tests.rs).

    python3 scripts/hand-checked-fixtures.py           # rewrite the fixtures
    python3 scripts/hand-checked-fixtures.py --check   # fail if they're out of date (CI)
"""

import sys
from decimal import ROUND_HALF_UP, Decimal, getcontext
from fractions import Fraction as Q
from pathlib import Path

getcontext().prec = 50
PI = Decimal("3.14159265358979323846264338327950288419716939937510")
OUT = Path(__file__).resolve().parent.parent / "fixtures" / "hand-checked"


def dec(q):
    """A fraction as a 50-digit decimal."""
    return Decimal(q.numerator) / Decimal(q.denominator)


def bead_area(width, height):
    """EasyGCode's bead cross-section: a stadium, (w - h) * h + pi * (h / 2)^2."""
    w, h = dec(max(width, height)), dec(height)
    return (w - h) * h + PI * (h / 2) * (h / 2)


def fixed(v, digits):
    """JavaScript's toFixed on the exact value: half away from zero."""
    return str(v.quantize(Decimal(1).scaleb(-digits), rounding=ROUND_HALF_UP))


def margin(v, digits):
    """How far v is from a rounding boundary, in units of the last digit (0.5 = safest)."""
    scaled = v.scaleb(digits)
    return abs(abs(scaled - scaled.to_integral_value(rounding="ROUND_FLOOR")) - Decimal("0.5"))


def spiral(layers, width, flow, center, travel_speed):
    """The spiral through `layers` (lists of exact (x, y, z) points), as G-code lines.

    The rules, as EasyCAD's `spiralize` documents them:
    - Opening turn: layer 1's points, climbing from the bottom layer (z = z0 + (z1 - z0) * u,
      u = j / m), extrusion ramping up (the move into point j at (u + (j - 1) / m) / 2).
    - Turn k (1 .. n - 1): point j is layer k's point j moved a fraction u = j / m of the way to
      layer k + 1's (the move into turn 1's first point finishes the ramp: (1 + (m - 1) / m) / 2).
    - Closing turn: flat on the top layer, back to its first point, extrusion ramping down
      (1 - (u + (j - 1) / m) / 2), the first move at full flow.
    - The local layer height at a point is the vertical gap to the turn below it: layer k's
      height over layer k - 1 there, blended like the points. A move is extruded for the mean of
      the local heights at its two ends.
    - E = bead area * length * extrusion factor (pellet printer: mm^3, multiplier 1).
    - F = round(flow * 60 / bead area), written when it changes.
    """
    n, m = len(layers) - 1, len(layers[0])
    gap = [[layers[k][j][2] - layers[k - 1][j][2] if k else Q(0) for j in range(m)] for k in range(n + 1)]
    moves = []  # (point, extrusion factor, local height)
    for j in range(m):
        u = Q(j, m)
        a, b = layers[0][j], layers[1][j]
        moves.append(((b[0], b[1], a[2] + (b[2] - a[2]) * u), (u + Q(j - 1, m)) / 2, gap[1][j]))
    for k in range(1, n):
        for j in range(m):
            u = Q(j, m)
            a, b = layers[k][j], layers[k + 1][j]
            p = tuple(a[i] + (b[i] - a[i]) * u for i in range(3))
            factor = (1 + Q(m - 1, m)) / 2 if (k, j) == (1, 0) else Q(1)
            moves.append((p, factor, gap[k][j] + (gap[k + 1][j] - gap[k][j]) * u))
    for j in range(m + 1):
        u = Q(j, m)
        if j == 0:
            factor = Q(1) if n > 1 else (1 + Q(m - 1, m)) / 2
        else:
            factor = max(Q(0), 1 - (u + Q(j - 1, m)) / 2)
        moves.append((layers[n][j % m], factor, gap[n][j % m]))

    cx, cy = center
    lines, notes, closest = [], [], Decimal("0.5")
    first, prev_feed = True, None
    (x0, y0, z0), _, last_h = moves[0]
    lines.append(f"G0 X{fixed(dec(x0 + cx), 3)} Y{fixed(dec(y0 + cy), 3)} Z{fixed(dec(z0), 3)} F{travel_speed * 60}")
    prev = moves[0][0]
    for i, (p, factor, h) in enumerate(moves[1:], start=1):
        if i == m + 1 and n > 1:
            lines.append("; (the fan would come on here, after the ramp turn)")
        hm, ends = (last_h + h) / 2, (last_h, h)
        last_h = h
        d2 = sum((p[k] - prev[k]) ** 2 for k in range(3))
        length = dec(d2).sqrt()
        area = bead_area(width, hm)
        e = area * length * dec(factor)
        feed_exact = Decimal(flow * 60) / area
        feed = int((feed_exact + Decimal("0.5")).to_integral_value(rounding="ROUND_FLOOR"))
        x, y, z = (dec(p[0] + cx), dec(p[1] + cy), dec(p[2]))
        line = f"G1 X{fixed(x, 3)} Y{fixed(y, 3)} Z{fixed(z, 3)} E{fixed(e, 5)}"
        if first or feed != prev_feed:
            line += f" F{feed}"
            prev_feed, first = feed, False
        note = (
            f"; height {fmt(hm)} (ends {fmt(ends[0])}, {fmt(ends[1])}), length sqrt({fmt(d2)}) = {length:.6f},"
            f" area {area:.6f}, factor {fmt(factor)}"
        )
        lines.append(f"{line} {note}")
        for v, digits in ((x, 3), (y, 3), (z, 3), (e, 5), (feed_exact, 0)):
            closest = min(closest, margin(v, digits))
        prev = p
    return lines, len(moves) - 1, closest


def fmt(q):
    """A fraction as a short decimal (exact when it terminates)."""
    d = dec(q).normalize()
    return f"{d:f}" if abs(d.as_tuple().exponent) <= 8 else f"{d:.8f}"


def tilted_diamond():
    """A diamond (corners on the axes, 10 mm out) whose top is cut by the plane z = 2 + 0.1 x,
    with 3 layers tweened between bottom and top: 4 turns whose local heights are the plane's
    height / 4 at each corner (0.75 at +X, 0.5 at +Y and -Y, 0.25 at -X)."""
    corners = [(Q(10), Q(0)), (Q(0), Q(10)), (Q(-10), Q(0)), (Q(0), Q(-10))]
    top = [2 + Q(1, 10) * x for x, _ in corners]
    layers = [[(x, y, top[j] * Q(k, 4)) for j, (x, y) in enumerate(corners)] for k in range(5)]
    return spiral(layers, width=Q(1), flow=5, center=(125, 125), travel_speed=100)


RECIPE = """easycad 0.1
base = polyline([[10, 0, 0], [0, 10, 0], [-10, 0, 0], [0, -10, 0]], closed: true)
top = base |> move(z: 2 + 0.1 * x)
layers = tween(base, top, range(0, 1, count: 5))
path = spiralize(layers, width: 1, flow: 5)
print path"""


EVENTS_RECIPE = """easycad 0.1
square = polyline([[0, 0], [20, 0], [20, 20], [0, 20]], closed: true)
two = toolpath(square, width: 1, layer: 0.5, flow: 4) |> stack(2)
tuned = adjust(two, speed: 1 - 0.5 * greater(x, 19), extrude: 1 + 0.5 * greater(y, 19))
ramped = accelerate(tuned, 10)
cooled = fan(ramped, level: 60 * greater(z, 0.6))
path = dwell(cooled, [[20, 20, 1]], 0.5)
print path"""


def machine_events():
    """A 20 mm square, two layers, tuned with the toolpath steps; worked out by their rules:

    - `toolpath` prints the square at z = 0.5 (one layer up), `stack` the copy at z = 1.0, with
      the hop between them: up to 1.5 (one layer above the new copy), over, down.
    - `adjust` reads its fields at each move's middle: the right side (middle x = 20) at half
      speed, the top side (middle y = 20) with 1.5 x extrusion.
    - `accelerate` (10 mm/s^2): the top side starts faster than the right side, from
      v0 = 2 / area to v1 = 4 / area mm/s (flow 4 mm^3/s), so its first
      (v1^2 - v0^2) / 20 = 0.6 / area^2 mm are cut into 8 equal pieces, piece k running at
      sqrt(v0^2 + 20 * k / 8 * 0.6 / area^2) = sqrt(4 + 1.5 k) / area; F = 60 x that.
    - `fan` (60 % above z = 0.6): M106 S153 (round(60 * 2.55)) before layer 2's first move;
      no automatic fan.
    - `dwell`: G4 P500 after the move ending at (20, 20, 1).
    """
    area = bead_area(Q(1), Q(1, 2))
    need = Decimal("0.6") / (area * area)
    cx = cy = 125
    lines, closest = [], Decimal("0.5")
    last_feed, first = None, True

    def travel(p):
        nonlocal last_feed
        lines.append(f"G0 X{fixed(Decimal(p[0]) + cx, 3)} Y{fixed(Decimal(p[1]) + cy, 3)} Z{fixed(Decimal(p[2]), 3)} F6000")
        last_feed = "travel"

    def move(a, b, speed, factor, note):
        nonlocal last_feed, first, closest
        length = sum((Decimal(b[k]) - Decimal(a[k])) ** 2 for k in range(3)).sqrt()
        e = area * length * dec(factor)
        feed_exact = speed * 60
        feed = int((feed_exact + Decimal("0.5")).to_integral_value(rounding="ROUND_FLOOR"))
        x, y, z = (Decimal(b[0]) + cx, Decimal(b[1]) + cy, Decimal(b[2]))
        line = f"G1 X{fixed(x, 3)} Y{fixed(y, 3)} Z{fixed(z, 3)} E{fixed(e, 5)}"
        if first or feed != last_feed:
            line += f" F{feed}"
            last_feed, first = feed, False
        lines.append(f"{line} ; {note}: length {length:.6f}, E = area x length x {fmt(factor)}, F = {feed_exact:.4f}")
        for v, digits in ((x, 3), (y, 3), (z, 3), (e, 5), (feed_exact, 0)):
            closest = min(closest, margin(v, digits))

    v1 = Decimal(4) / area
    for layer, z in ((1, Decimal("0.5")), (2, Decimal("1.0"))):
        if layer == 1:
            travel((0, 0, z))
        else:
            lines.append("; hop: up to one layer above the new copy, over, down")
            travel((0, 0, Decimal("1.5")))
            travel((0, 0, Decimal("1.5")))
            travel((0, 0, z))
            lines.append("M106 S153 ; fan 60 % from layer 2 on")
        move((0, 0, z), (20, 0, z), v1, Q(1), "bottom side, full speed")
        move((20, 0, z), (20, 20, z), v1 / 2, Q(1), "right side, half speed")
        if layer == 2:
            lines.append("G4 P500 ; dwell at (20, 20, 1)")
        prev = (Decimal(20), Decimal(20), z)
        for k in range(1, 8):
            mark = (Decimal(20) - need * k / 8, Decimal(20), z)
            speed = (Decimal(4) + Decimal("1.5") * k).sqrt() / area
            move(prev, mark, speed, Q(3, 2), f"top side, ramp piece {k} of 8")
            prev = mark
        move(prev, (0, 20, z), v1, Q(3, 2), "top side, piece 8 and the rest at full speed")
        move((0, 20, z), (0, 0, z), v1, Q(1), "left side, full speed")
    return lines, closest


def main():
    lines, count, closest = tilted_diamond()
    header = [
        "; Hand-checked fixture: spiralize through tilted layers (scripts/hand-checked-fixtures.py).",
        "; The recipe (Klipper pellet printer, no start/end blocks, bed centre 125, 125):",
        *[f";   {l}" for l in RECIPE.splitlines()],
        "; Each move's comment: its layer height (mean of the gaps at its two ends), length, bead",
        "; area (w - h) h + pi (h/2)^2 and extrusion factor; E = area * length * factor (mm^3),",
        "; F = round(5 * 60 / area).",
        "G90",
        "M83",
    ]
    text = "\n".join(header + lines) + "\n"
    events, events_closest = machine_events()
    events_text = "\n".join(
        [
            "; Hand-checked fixture: toolpath tuning steps (scripts/hand-checked-fixtures.py).",
            "; The recipe (Klipper pellet printer, bed centre 125, 125; the moves between the",
            "; start and end blocks, which are EasyGCode's and golden-tested elsewhere):",
            *[f";   {l}" for l in EVENTS_RECIPE.splitlines()],
            "; Bead area (w - h) h + pi (h/2)^2 = 0.25 + pi / 16 mm^2 for every move.",
            "G90",
            "M83",
            *events,
        ]
    ) + "\n"
    files = {OUT / "spiralize-tilted.gcode": text, OUT / "machine-events.gcode": events_text}
    if "--check" in sys.argv:
        for target, content in files.items():
            if not target.exists() or target.read_text() != content:
                print(f"{target} is out of date; run python3 scripts/hand-checked-fixtures.py")
                sys.exit(1)
        print("hand-checked fixtures are up to date")
        return
    OUT.mkdir(parents=True, exist_ok=True)
    for target, content in files.items():
        target.write_text(content)
    print(f"wrote spiralize-tilted.gcode ({count} printing moves); closest call to a rounding boundary: {closest:.3e} of a digit")
    print(f"wrote machine-events.gcode; closest call to a rounding boundary: {events_closest:.3e} of a digit")


if __name__ == "__main__":
    main()
