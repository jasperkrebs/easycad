#!/usr/bin/env python3
"""An animated robot simulation of an EasyCAD robot program, in the SPHR handover's own viewer.

The handover's `Non_Planar_Demo.html` is its `Globe_E27_Smoothed_Roll/reference/viewer_template.html`
filled by `build_trumpet.py` with a replay of its program: one row per moment (position, time,
speed, extruder, fans, heaters, phase, layer, source line, joint angles, orientation) and a
summary (`META`: the robot's geometry, the tool and arm proxies, the bed, the layers' times).
This replays a program EasyCAD wrote (crates/core/src/krl.rs) the same way and fills the same
template, so the viewer shows the arm, the tool and the print as it grows: play, scrub, orbit,
"Whole robot", layer by layer.

The replay is nominal, as the handover's: straight between targets at the programmed speed (the
controller's spline, acceleration and blending aren't modelled), rotations alone at 30 °/s.
Layers are counted by turns around the print's centre (the program has no layer markers).

    python3 scripts/robot-viewer.py EASYCAD.SRC --title "Sphere: first 40 mm" --output sphere.html
"""

import argparse
import base64
import gzip
import json
import math
import re
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent / "SPHR_Full_Print_Handover-2"
REFERENCE = ROOT / "Globe_E27_Smoothed_Roll" / "reference"
CHECKS = ROOT / "Globe_E27_30_Layers"

POSE = re.compile(r"\{X (-?[\d.]+),Y (-?[\d.]+),Z (-?[\d.]+),A (-?[\d.]+),B (-?[\d.]+),C (-?[\d.]+)\}")
SEND = re.compile(r'EKI_SEND\("[^"]+", "([^#]+)#"\)')


def rot(a, b, c):
    a, b, c = np.radians([a, b, c])
    ca, sa, cb, sb, cc, sc = np.cos(a), np.sin(a), np.cos(b), np.sin(b), np.cos(c), np.sin(c)
    return np.array([
        [ca * cb, ca * sb * sc - sa * cc, ca * sb * cc + sa * sc],
        [sa * cb, sa * sb * sc + ca * cc, sa * sb * cc - ca * sc],
        [-sb, cb * sc, cb * cc],
    ])


def quaternion(a, b, c):
    """KUKA's A, B, C (ZYX) as a quaternion w, x, y, z (as build_trumpet.py)."""
    ha, hb, hc = np.radians([a, b, c]) / 2
    sa, sb, sc = np.sin([ha, hb, hc])
    ca, cb, cc = np.cos([ha, hb, hc])
    return [ca * cb * cc + sa * sb * sc, ca * cb * sc - sa * sb * cc,
            ca * sb * cc + sa * cb * sc, sa * cb * cc - ca * sb * sc]


def replay(src, base):
    """Rows as build_trumpet.py's: x, y, z (world), time, speed, screw, fan 1, extruding, phase,
    layer (0 for now), source line, T0, T1, T2, fan 2; and each row's A, B, C."""
    lines = src.splitlines()
    # The trigger subroutines: name -> what they send.
    subs, name = {}, None
    for s in lines:
        m = re.match(r"DEF (\w+)\(\)", s)
        if m:
            name = m[1]
        elif name and (m := SEND.search(s)):
            subs[name] = m[1]
    state = dict(speed=0.0, flow=0, fan1=0, on=0, phase=0, temps=[0, 0, 0], fan2=0)
    now = 0.0
    xyz = abc = None
    rows, poses = [], []
    pending = []  # trigger payloads that fire with the next motion

    def row(n, velocity=None):
        if xyz is None:
            return
        rows.append([*xyz, now, state["speed"] if velocity is None else velocity, state["flow"],
                     state["fan1"], state["on"], state["phase"], 0, n, *state["temps"], state["fan2"]])
        poses.append(abc)

    def command(payload, n):
        cmd, *args = payload.split()
        if cmd == "STOP_EXTRUDE":
            state["on"] = 0
            state["phase"] = 2 if state["phase"] == 1 else state["phase"]
        elif cmd == "START_EXTRUDE":
            state["on"] = 1
            state["phase"] = 3  # the purge
        elif cmd == "CHANGE_SPEED1":
            state["flow"] = int(args[0])
        elif cmd == "SET_FAN1":
            state["fan1"] = int(args[0])
        elif cmd == "SET_FAN2":
            state["fan2"] = int(args[0])
        elif cmd == "SET_TEMP":
            state["temps"][int(args[0][1])] = int(args[1])
        row(n, 0)

    for n, raw in enumerate(lines, 1):
        s = raw.strip()
        if s == "END":
            break
        if s.startswith("SPLINE"):
            state["phase"] = 1
        elif s in (";LIFT", ";UPRIGHT"):
            state["phase"] = 4
        elif s.startswith("$VEL.CP"):
            state["speed"] = float(s.split("=")[-1]) * 1000
        elif m := SEND.search(s):
            command(m[1], n)
        elif s.startswith("TRIGGER"):
            pending.append(subs[re.search(r"DO (\w+)\(\)", s)[1]])
        elif s.startswith("WAIT SEC"):
            row(n, 0)
            now += float(s.split()[-1])
            row(n, 0)
        elif s.startswith(("LIN ", "SPL ")):
            m = POSE.search(s)
            if not m:
                continue
            x, y, z, a, b, c = map(float, m.groups())
            if s.startswith("SPL") and (v := re.search(r"\$VEL\.CP=([\d.]+)", s)):
                state["speed"] = float(v[1]) * 1000
            for p in pending:
                command(p, n)
            pending = []
            dest = [x + base[0], y + base[1], z + base[2]]
            if xyz is not None:
                row(n)
                d = math.dist(dest, xyz)
                if d:
                    now += d / max(state["speed"], 1e-6)
                else:
                    r = rot(a, b, c) @ rot(*abc).T
                    ang = math.degrees(math.acos(np.clip((np.trace(r) - 1) / 2, -1, 1)))
                    now += ang / 30 + 0.2 if ang else 0.0
            xyz, abc = dest, (a, b, c)
            row(n)
    return np.array(rows, dtype=float), np.array(poses, dtype=float)


def layers_by_turns(rows, centre):
    """Layer 1, 2, … by whole turns around the centre, counted while printing."""
    printing = (rows[:, 8] == 1) & (rows[:, 7] == 1)
    angle = np.arctan2(rows[:, 1] - centre[1], rows[:, 0] - centre[0])
    turns = np.zeros(len(rows))
    total, last = 0.0, None
    for i in range(len(rows)):
        if printing[i]:
            if last is not None:
                d = (angle[i] - last + math.pi) % (2 * math.pi) - math.pi
                total += d
            last = angle[i]
            turns[i] = abs(total) / (2 * math.pi)
    layer = np.where(printing, np.floor(turns) + 1, 0)
    # After the print: the last layer (so the finish shows with the whole print).
    top = int(layer.max()) if printing.any() else 1
    first = int(np.argmax(printing)) if printing.any() else 0
    layer[first:][~printing[first:]] = np.maximum.accumulate(layer[first:])[~printing[first:]]
    return layer, top


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("src", type=Path)
    ap.add_argument("--base", type=float, nargs=3, default=[0.0, 50.0, -0.3], help="TR_BASE X Y Z (no rotation)")
    ap.add_argument("--centre", type=float, nargs=2, default=[1088.304936, -115.143158], help="the print's centre, world X Y")
    ap.add_argument("--title", default="EasyCAD robot program")
    ap.add_argument("--subtitle", default="Nozzle along the wall, up to 45° | fan to part 5 mm | fan to bed 10 mm")
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()

    sys.dont_write_bytecode = True  # leave the handover folder as it is
    sys.path.insert(0, str(CHECKS))
    from kinematics import A, AL, DD, TOOL, inverse  # noqa: E402

    qa = json.loads((CHECKS / "QA.json").read_text())
    cad = json.loads((CHECKS / "cad_proxy.json").read_text())
    rows, abc = replay(args.src.read_text(), args.base)
    rotations = np.array([rot(*p) for p in abc])
    joints = inverse(rows[:, :3], rotations)
    quats = np.array([quaternion(*p) for p in abc])
    layer, top = layers_by_turns(rows, args.centre)
    rows[:, 9] = layer
    # Each layer's start and end time.
    ranges = [None] * (top + 1)
    for i in np.flatnonzero((rows[:, 8] == 1) & (rows[:, 7] == 1)):
        l = int(rows[i, 9])
        t = float(rows[i, 3])
        ranges[l] = [t, t] if ranges[l] is None else [ranges[l][0], t]
    for l in range(1, top + 1):
        if ranges[l] is None:
            ranges[l] = ranges[l - 1]
    purge = next((rows[i, :3].tolist() for i in range(len(rows)) if rows[i, 8] == 3), rows[0, :3].tolist())
    limits = np.array([[-185, 185], [-175, 60], [-120, 170], [-180, 180], [-125, 125]])
    margin = np.minimum(joints[:, :5] - limits[:, 0], limits[:, 1] - joints[:, :5])
    meta = dict(
        pointCount=len(rows), modelMotions=int(((rows[:, 8] == 1) & (rows[:, 7] == 1)).sum()),
        layers=top, layerRanges=ranges, nominalSeconds=float(np.float32(rows[-1, 3])), srcCount=1,
        purgeXYZ=purge, wallConnections=[], a=A.tolist(), d=DD.tolist(), alpha=AL.tolist(),
        tool=TOOL.tolist(), cad=cad, bedBoundsWorldMm=qa["bedBoundsWorldMm"],
        bedClearanceTargetMm=qa["bedClearanceTargetMm"], bedExtraToolProxies=qa["bedExtraToolProxies"],
        collisionExcludedToolParts=qa["collisionExcludedToolParts"],
        jointLimitViolationVertices=int(np.any(margin < 0, axis=1).sum()),
        minimumJointMarginsDeg=margin.min(axis=0).tolist(),
    )
    data = np.column_stack([rows, joints, quats]).astype("<f4")
    assert data.shape[1] == 25
    packed = base64.b64encode(gzip.compress(data.tobytes())).decode()
    html = (REFERENCE / "viewer_template.html").read_text(encoding="utf-8")
    title = args.title.replace("<", "&lt;")
    html = html.replace("<title>E27 216 mm Trumpet | Robot printing simulation</title>", f"<title>{title} | Robot printing simulation</title>")
    html = html.replace("<h1>E27 Globe - inferred profile</h1>", f"<h1>{title}</h1>")
    html = html.replace("<p>Tilt held steady per turn, up to 45 deg | fan to part 5 mm | fan to bed 10 mm</p>", f"<p>{args.subtitle.replace('<', '&lt;')}</p>")
    html = html.replace("The nozzle points toward the previous spiral turn with a 45° tilt cap and local tilt relief.",
                        "The nozzle follows the wall (EasyCAD's tilt step), at most 45° from upright and less near the bed. Layers are counted by turns around the print's centre.")
    html = html.replace("__LAYER_COUNT__", str(top)).replace("__METADATA__", json.dumps(meta, separators=(",", ":"))).replace("__PACKED_DATA__", packed)
    args.output.write_text(html, encoding="utf-8")
    print(f"{args.output}: {len(rows)} rows, {top} layers, {rows[-1, 3] / 60:.1f} min nominal, joint-limit violations {meta['jointLimitViolationVertices']}, {args.output.stat().st_size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
