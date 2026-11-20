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
    - F = round(flow * 60 / bead area), written whenever the exact value changes.
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
        if first or feed_exact != prev_feed:
            line += f" F{feed}"
            prev_feed, first = feed_exact, False
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
        if first or feed_exact != last_feed:
            line += f" F{feed}"
            last_feed, first = feed_exact, False
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


FILAMENT_RECIPE = """easycad 0.1
square = polyline([[0, 0], [20, 0], [20, 20], [0, 20]], closed: true)
layer1 = toolpath(square, width: 0.6, layer: 0.3, flow: 6) |> temperature(nozzle: 230, at_end: true)
primer = toolpath(line([-20, -20], [-10, -20]), width: 0.6, layer: 0.3, speed: 20)
  |> wait_temperature(nozzle: 230)
  |> extrusion_override(80, at_end: true)
  |> speed_override(125, at_end: true)
layer2 = toolpath(square |> move(z: 0.3), width: 0.6, layer: 0.3, flow: 6)
  |> speed_override(100, at_end: true)
  |> extrusion_override(100, at_end: true)
  |> temperature(at_end: true)
path = join([layer1, primer, layer2], clear: 1)
print path"""


def filament_machine():
    """Machine events on the filament printer (nozzle 210 C in its settings, part fan 100 %,
    filament 1.75 mm, travel 100 mm/s), worked out by the steps' rules:

    - `temperature(nozzle: 230, at_end: true)`: M104 S230 after layer 1's last move;
      `temperature(at_end: true)` with no temperature: back to the printer's own, M104 S210.
    - `wait_temperature(nozzle: 230)` on the primer: M109 S230 after the travel to its start,
      before it prints.
    - `extrusion_override` / `speed_override`: M221 / M220 S<whole percent>, in the order added.
    - `join(..., clear: 1)`: up to z = 1, over, down to the next part's start. The primer starts
      no higher than layer 1 (z = 0.3), so it belongs to the first layer; layer 2 (z = 0.6)
      doesn't, so the automatic fan (M106 S255) comes on right before layer 2's first point,
      the move down onto it.
    - E in mm of filament: bead area x length / (pi x 0.875^2); F = round(6 x 60 / area) at a
      flow of 6 mm^3/s, 1200 for the primer's 20 mm/s.
    """
    area = bead_area(Q(3, 5), Q(3, 10))
    filament = PI * Decimal("0.875") * Decimal("0.875")
    cx = cy = 125
    lines, closest = ["G90", "M83"], Decimal("0.5")
    state = {"feed": None, "first": True}

    def travel(x, y, z, note=""):
        lines.append(f"G0 X{fixed(Decimal(x) + cx, 3)} Y{fixed(Decimal(y) + cy, 3)} Z{fixed(Decimal(z), 3)} F6000{note}")
        state["feed"] = "travel"

    def move(a, b, feed_exact, note):
        nonlocal closest
        length = sum((Decimal(b[k]) - Decimal(a[k])) ** 2 for k in range(3)).sqrt()
        e = area * length / filament
        feed = int((feed_exact + Decimal("0.5")).to_integral_value(rounding="ROUND_FLOOR"))
        x, y, z = (Decimal(b[0]) + cx, Decimal(b[1]) + cy, Decimal(b[2]))
        line = f"G1 X{fixed(x, 3)} Y{fixed(y, 3)} Z{fixed(z, 3)} E{fixed(e, 5)}"
        if state["first"] or feed_exact != state["feed"]:
            line += f" F{feed}"
            state["feed"], state["first"] = feed_exact, False
        lines.append(f"{line} ; {note}: length {length:.6f}, E = area x length / filament area, F = {feed_exact:.4f}")
        for v, digits in ((x, 3), (y, 3), (z, 3), (e, 5), (feed_exact, 0)):
            closest = min(closest, margin(v, digits))

    layer_feed = Decimal(360) / area
    corners = [(0, 0), (20, 0), (20, 20), (0, 20), (0, 0)]

    def square(z, label):
        for k in range(1, len(corners)):
            a, b = corners[k - 1], corners[k]
            move((a[0], a[1], z), (b[0], b[1], z), layer_feed, f"{label}, side {k}")

    z1, z2 = Decimal("0.3"), Decimal("0.6")
    travel(0, 0, z1)
    square(z1, "layer 1")
    lines.append("M104 S230 ; temperature(nozzle: 230, at_end: true)")
    travel(0, 0, 1, " ; hop to the primer: up to 1")
    travel(-20, -20, 1, " ; over")
    travel(-20, -20, z1, " ; down onto the primer's start")
    lines.append("M109 S230 ; wait_temperature(nozzle: 230)")
    move((-20, -20, z1), (-10, -20, z1), Decimal(20 * 60), "primer")
    lines.append("M221 S80 ; extrusion_override(80, at_end: true)")
    lines.append("M220 S125 ; speed_override(125, at_end: true)")
    travel(-10, -20, 1, " ; hop to layer 2: up to 1")
    travel(0, 0, 1, " ; over")
    lines.append("M106 S255 ; the automatic fan, right before layer 2's first point")
    travel(0, 0, z2, " ; down onto layer 2's start")
    square(z2, "layer 2")
    lines.append("M220 S100 ; speed_override(100, at_end: true)")
    lines.append("M221 S100 ; extrusion_override(100, at_end: true)")
    lines.append("M104 S210 ; temperature(at_end: true): the printer's own")
    return lines, closest


# ---- A robot program (KRL) for the KUKA cell of the SPHR handover (crates/core/src/krl.rs).

KRL_ORIGIN = (Q("1088.304936"), Q("-115.143158"), Q(-659))
KRL_BASE = (Q(0), Q(50), Q("-0.3"))
KRL_SCREW = 30  # screw units per mm^3/s
KRL_MAX_TURN = 30  # deg/s


def krl_two(v):
    """Two decimals, half away from zero, never -0.00 (v a float: its exact value)."""
    d = Decimal(v).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
    return "0.00" if d == 0 else str(d)


def krl_pose(p, n):
    """The pose of the nozzle tip at world p (exact), pointing along n (floats): z along n, x
    world -Y made square to it, y = z cross x; A, B, C as KUKA's ZYX angles of that matrix, in
    the base frame (a pure shift here: the handover's TR_BASE has no rotation)."""
    import math
    z = n
    yd = (0.0, -1.0, 0.0)
    k = sum(a * b for a, b in zip(z, yd))
    x = tuple(a - k * b for a, b in zip(yd, z))
    lx = math.sqrt(sum(a * a for a in x))
    x = tuple(a / lx for a in x)
    y = (z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0])
    r00, r10, r20 = x
    r21, r22 = y[2], z[2]
    a = math.degrees(math.atan2(r10, r00))
    b = math.degrees(math.atan2(-r20, math.hypot(r00, r10)))
    c = math.degrees(math.atan2(r21, r22))
    pos = [dec(p[i] - KRL_BASE[i]) for i in range(3)]
    parts = [str(v.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)) for v in pos]
    parts = ["0.00" if Decimal(v) == 0 else v for v in parts]
    angles = [krl_two(v) for v in (a, b, c)]
    return "{X %s,Y %s,Z %s,A %s,B %s,C %s}" % tuple(parts + angles), [*pos], [a, b, c]


def robot_square():
    """A square on the bed, then up a layer onto a second one, the nozzle tilting on the way.

    The toolpath (built in krl.rs's test, not from a recipe): a line of width 5 and layer 2.5
    at 10 mm/s through the points below (as drawn; the nozzle is a layer higher), the nozzle
    upright for the first four and then along the axes given. The program's rules (krl.rs):
    - each point is the origin's world position plus the point, written less the base frame;
    - each move at 10 mm/s, at most 25, and at most 30 deg/s of wrist turn: speed <= 30 * length
      / turn, written to 0.1 mm/s rounded down;
    - the screw command is round(bead area * speed * 30), bead area (w - h) h + pi (h/2)^2;
    - the part fan (255) comes on at the first point a layer above the first;
    - tolerance 0: every point is kept.
    """
    import math
    h = Q(5, 2)
    drawn = [(0, 0, 0), (20, 0, 0), (20, 20, 0), (0, 20, 0), (0, 0, h), (20, 0, h), (20, Q(41, 10), h)]
    pts = [(Q(x), Q(y), Q(z) + h) for x, y, z in drawn]
    s30, c30 = 0.5, math.sqrt(3) / 2
    s20, c20 = math.sin(math.radians(20)), math.cos(math.radians(20))
    up = (0.0, 0.0, 1.0)
    axes = [up, up, up, up, (s30, 0.0, c30), (0.0, s30, c30), (0.0, -s20, c20)]
    # The turn between the nozzle's directions, worked out by hand (all in the XZ or YZ planes,
    # or with the cosine of the angle between two 30-degree leans 90 degrees apart: cos^2 30).
    turns = [None, 0, 0, 0, 30, math.degrees(math.acos(0.75)), 50]
    area = bead_area(5, h)
    world = [tuple(KRL_ORIGIN[i] + p[i] for i in range(3)) for p in pts]
    speed, command, notes = [None], [None], [None]
    for i in range(1, len(pts)):
        length = math.sqrt(sum(float(pts[i][k] - pts[i - 1][k]) ** 2 for k in range(3)))
        v = 10.0
        if turns[i]:
            v = min(v, KRL_MAX_TURN * length / turns[i])
        v = math.floor(v * 10 + 1e-9) / 10
        speed.append(v)
        c = area * Decimal(str(v)) * KRL_SCREW
        command.append(int(c.quantize(Decimal(1), rounding=ROUND_HALF_UP)))
        notes.append(f"length {length:.4f}, turn {turns[i] or 0:.3f} deg, speed {v} mm/s, command {c:.3f}")
    send = lambda s: f'RET = EKI_SEND("EthernetKRL_Config", "{s}#")'
    pose = lambda i: krl_pose(world[i], axes[i])[0]
    lines = [
        "&ACCESS RVP",
        "DEF EASYCAD()",
        "; EasyCAD: Robot square",
        "DECL EKI_STATUS RET",
        "DECL SPL_VEL_MODE EC_MODE",
        "DECL SPL_VEL_RESTR_STRUC EC_SAVED, EC_RESTR",
        "BAS (#INITMOV,0)",
        "$ACC.ORI1 = 150.0",
        "$ACC.ORI2 = 150.0",
        "$VEL.ORI1 = 200.0",
        "$VEL.ORI2 = 200.0",
        "$ADVANCE = 3",
        send("STOP_EXTRUDE"),
        "$TOOL = TR_TOOL",
        "; Reuse tool 6 inertia and inertia-axis orientation; override mass and COM XYZ.",
        "$LOAD = LOAD_DATA[6]",
        "$LOAD.M = TR_MASS",
        "$LOAD.CM.X = TR_COM.X",
        "$LOAD.CM.Y = TR_COM.Y",
        "$LOAD.CM.Z = TR_COM.Z",
        "$BASE = TR_BASE",
        "$IPO_MODE = #BASE",
        "$ORI_TYPE = #VAR",
        "; SIMULATION ONLY - verify cell, cables and controller.",
        "PTP $AXIS_ACT",
        send("SET_TEMP T0 220"),
        send("SET_TEMP T1 190"),
        send("SET_TEMP T2 70"),
        send("SET_FAN1 0"),
        send("SET_FAN2 5"),
        "HALT",
        "$VEL.CP = 0.05000",
        "LIN " + krl_pose((Q("1088.304936"), Q("134.856842"), Q(-590)), up)[0],
        "HALT",
        "WAIT SEC 0",
        send("CHANGE_SPEED1 12000"),
        send("START_EXTRUDE"),
        "WAIT SEC 20",
        send(f"CHANGE_SPEED1 {command[1]}"),
        "$VEL.CP = 0.10000",
        "LIN " + krl_pose((world[0][0], world[0][1], world[0][2] + 5), up)[0],
        "$VEL.CP = 0.02000",
        "LIN " + pose(0),
        "EC_MODE = $SPL_VEL_MODE",
        "EC_SAVED = $SPL_VEL_RESTR",
        "EC_RESTR = EC_SAVED",
        "$SPL_VEL_MODE = #OPT",
        "EC_RESTR.ORI_VEL = #ON",
        "EC_RESTR.ORI_ACC = #ON",
        "EC_RESTR.CART_ACC = #ON",
        "$SPL_VEL_RESTR = EC_RESTR",
        "SPLINE WITH $ACC={CP 0.050, ORI1 150.0, ORI2 150.0}",
    ]
    helpers = []
    def trigger(instruction):
        if instruction not in helpers:
            helpers.append(instruction)
        n = helpers.index(instruction) + 1
        lines.append(f"  TRIGGER WHEN PATH=0 ONSTART DELAY=0 DO EASYCAD_EVT_{n}() PRIO=-1")
    current = command[1]
    fan_at = next(i for i in range(1, len(pts)) if pts[i][2] >= pts[1][2] + h)
    for i in range(1, len(pts)):
        if command[i] != current:  # 3348 -> 803: far more than 2 %
            current = command[i]
            trigger(send(f"CHANGE_SPEED1 {current}"))
        if i == fan_at:
            trigger(send("SET_FAN1 255"))
        lines.append(f"  SPL {pose(i)} WITH $VEL.CP={Decimal(str(speed[i])) / 1000:.5f}")
    last = world[-1]
    lifted = (last[0], last[1], last[2] + 50)
    lines += [
        "ENDSPLINE",
        ";END PRINT",
        "WAIT SEC 0",
        send("STOP_EXTRUDE"),
        send("CHANGE_SPEED1 0"),
        send("SET_TEMP T0 0"),
        send("SET_TEMP T1 0"),
        send("SET_TEMP T2 0"),
        ";LIFT",
        "$VEL.CP = 0.05000",
        "LIN " + krl_pose(lifted, axes[-1])[0],
        ";UPRIGHT",
        "$VEL.ORI1 = 30.0",
        "$VEL.ORI2 = 30.0",
        "LIN " + krl_pose(lifted, up)[0],
        "WAIT SEC 0",
        send("SET_FAN1 0"),
        send("SET_FAN2 0"),
        "$SPL_VEL_RESTR = EC_SAVED",
        "$SPL_VEL_MODE = EC_MODE",
        "END",
    ]
    for n, h_ in enumerate(helpers, 1):
        lines += ["", f"DEF EASYCAD_EVT_{n}()", "DECL EKI_STATUS RET", h_, "END"]
    # How close the angles came to a rounding boundary (in units of the last digit).
    closest = min(
        margin(Decimal(v), 2)
        for i in range(len(pts))
        for v in krl_pose(world[i], axes[i])[2]
    )
    header = [
        ";; Hand-checked fixture: a robot program (KRL) for the SPHR cell (scripts/hand-checked-fixtures.py).",
        ";; The toolpath is built in crates/core/src/krl.rs's test; the robot's settings are the",
        ";; handover's defaults with tolerance 0. Lines starting ';;' aren't part of the program.",
        *[f";; move {i}: {notes[i]}" for i in range(1, len(pts))],
        f";; bead area (w - h) h + pi (h/2)^2 = {area:.6f} mm^2",
    ]
    return "\n".join(header + lines) + "\n", closest


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
    filament, filament_closest = filament_machine()
    filament_text = "\n".join(
        [
            "; Hand-checked fixture: machine events on the filament printer (scripts/hand-checked-fixtures.py).",
            "; The recipe (filament printer: nozzle 210 C, part fan 100 %, filament 1.75 mm, bed centre",
            "; 125, 125; the moves between the start and end blocks, which are EasyGCode's):",
            *[f";   {l}" for l in FILAMENT_RECIPE.splitlines()],
            "; Bead area (w - h) h + pi (h/2)^2 = 0.09 + 0.0225 pi mm^2 for every move.",
            *filament,
        ]
    ) + "\n"
    robot, robot_closest = robot_square()
    files = {
        OUT / "robot-square.src": robot,
        OUT / "spiralize-tilted.gcode": text,
        OUT / "machine-events.gcode": events_text,
        OUT / "filament-machine.gcode": filament_text,
    }
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
    print(f"wrote robot-square.src; closest call to a rounding boundary: {robot_closest:.3e} of a digit")
    print(f"wrote filament-machine.gcode; closest call to a rounding boundary: {filament_closest:.3e} of a digit")


if __name__ == "__main__":
    main()
