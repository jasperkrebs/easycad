#!/usr/bin/env python3
"""Replays an EasyCAD robot program (KRL) on the SPHR handover's robot model and checks it.

The handover (SPHR_Full_Print_Handover-2/) checks its own globe with
Globe_E27_30_Layers/rough_checks.py, which reads the replay out of the globe's HTML and models the
printed material from the globe's profile. This does the same checks for any program EasyCAD
writes (crates/core/src/krl.rs), with the handover's own robot model and geometry:

- kinematics.py: the KR50 R2500's inverse and forward kinematics and joint limits;
- proxy_collision.py, cad_proxy.json: the convex proxies of the tool and the arm links, the
  pedestal, and the separating-axis test between them;
- fan_pair_coordinates.json: the three nozzle-fan outlets, each with a 28 mm housing behind it,
  as rough_checks.py adds them; the heater barrel left out, as the handover does;
- QA.json: the bed (world z -659 and its rectangle) and the 10 mm bed clearance target.

The program's poses (every LIN and SPL target, in order, plus three samples between each two:
straight for the position, slerp for the orientation, as rough_checks.py) are turned into joint
angles, and then:

1. joint limits (A1-A5; A6 is continuous) and positions out of reach;
2. tool against arm, arm against itself (non-adjacent links), everything against the pedestal;
3. every tool part and arm link against the bed: at least 10 mm (the nozzle excepted);
4. every tool part (not the nozzle, not the heater barrel) and the wrist links against the
   material already printed: the program's own bead centreline (its SPL targets, resampled every
   2 mm), a round bead of `--width` mm, leaving out the last `--recent` mm behind the nozzle (the
   bead being laid). The fans should keep 5 mm, as the handover's target.

What it can't see, as the handover says of its own: the controller's spline (between targets the
path is taken as straight), cables and hoses, cell fixtures, the move from wherever the robot
starts (PTP $AXIS_ACT), sag of the print.

    python3 -m pip install -r SPHR_Full_Print_Handover-2/Globe_E27_30_Layers/requirements.txt
    python3 scripts/robot-check.py EASYCAD.SRC --width 5 --output report.json
"""

import argparse
import json
import math
import re
import sys
from pathlib import Path

import numpy as np

HANDOVER = Path(__file__).resolve().parent.parent / "SPHR_Full_Print_Handover-2" / "Globe_E27_30_Layers"


def load_handover(folder):
    sys.path.insert(0, str(folder))
    import kinematics  # noqa: E402
    import proxy_collision  # noqa: E402

    cad = json.loads((folder / "cad_proxy.json").read_text())
    qa = json.loads((folder / "QA.json").read_text())
    fans = json.loads((folder / "fan_pair_coordinates.json").read_text())["providedPairs"]
    return kinematics, proxy_collision, cad, qa, fans


def parse(src_text):
    """The program's poses in order: (line, x, y, z, a, b, c, printing), base-frame numbers."""
    base = None
    poses = []
    printing = False
    pose_re = re.compile(r"\{X (-?[\d.]+),Y (-?[\d.]+),Z (-?[\d.]+),A (-?[\d.]+),B (-?[\d.]+),C (-?[\d.]+)\}")
    for n, line in enumerate(src_text.splitlines(), 1):
        s = line.strip()
        if s.startswith("SPLINE"):
            printing = True
        elif s == ";END PRINT":
            printing = False
        if s.startswith(("LIN ", "SPL ")):
            m = pose_re.search(s)
            if m:
                poses.append((n, *map(float, m.groups()), printing and s.startswith("SPL")))
    return poses


def rot(a, b, c):
    a, b, c = np.radians([a, b, c])
    ca, sa, cb, sb, cc, sc = np.cos(a), np.sin(a), np.cos(b), np.sin(b), np.cos(c), np.sin(c)
    return np.array([
        [ca * cb, ca * sb * sc - sa * cc, ca * sb * cc + sa * sc],
        [sa * cb, sa * sb * sc + ca * cc, sa * sb * cc - ca * sc],
        [-sb, cb * sc, cb * cc],
    ])


def to_quaternion(r):
    w = math.sqrt(max(0.0, 1 + r[0, 0] + r[1, 1] + r[2, 2])) / 2
    x = math.copysign(math.sqrt(max(0.0, 1 + r[0, 0] - r[1, 1] - r[2, 2])) / 2, r[2, 1] - r[1, 2])
    y = math.copysign(math.sqrt(max(0.0, 1 - r[0, 0] + r[1, 1] - r[2, 2])) / 2, r[0, 2] - r[2, 0])
    z = math.copysign(math.sqrt(max(0.0, 1 - r[0, 0] - r[1, 1] + r[2, 2])) / 2, r[1, 0] - r[0, 1])
    q = np.array([w, x, y, z])
    return q / np.linalg.norm(q)


def from_quaternions(q):
    w, x, y, z = (q / np.linalg.norm(q, axis=1)[:, None]).T
    return np.column_stack([
        1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y),
        2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x),
        2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y),
    ]).reshape(-1, 3, 3)


def samples(poses, base_xyz):
    """Every target and three samples between each two: world xyz, rotation, source line,
    whether printing, and the distance along the printed path at that sample."""
    xyz = np.array([p[1:4] for p in poses]) + base_xyz
    quats = np.array([to_quaternion(rot(*p[4:7])) for p in poses])
    for i in range(1, len(quats)):
        if np.dot(quats[i], quats[i - 1]) < 0:
            quats[i] = -quats[i]
    lines = np.array([p[0] for p in poses])
    printing = np.array([p[7] for p in poses])
    out_xyz, out_q, out_line, out_print = [xyz[0]], [quats[0]], [lines[0]], [printing[0]]
    for i in range(1, len(poses)):
        q0, q1 = quats[i - 1], quats[i]
        arc = math.acos(min(1.0, abs(float(np.dot(q0, q1)))))
        for f in (0.25, 0.5, 0.75, 1.0):
            if arc > 1e-6:
                a, b = math.sin((1 - f) * arc) / math.sin(arc), math.sin(f * arc) / math.sin(arc)
            else:
                a, b = 1 - f, f
            out_xyz.append((1 - f) * xyz[i - 1] + f * xyz[i])
            out_q.append(a * q0 + b * q1)
            out_line.append(lines[i])
            out_print.append(printing[i])
    xyz = np.array(out_xyz)
    printing = np.array(out_print)
    # Distance along the printed path (only printing moves count).
    step = np.r_[0.0, np.linalg.norm(np.diff(xyz, axis=0), axis=1)]
    along = np.cumsum(np.where(printing, step, 0.0))
    return xyz, from_quaternions(np.array(out_q)), np.array(out_line), printing, along


def fan_parts(fans):
    faces = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7]]
    for i in range(4):
        j = (i + 1) % 4
        faces.extend([[i, j, j + 4], [i, j + 4, i + 4]])
    parts = []
    for outlet in fans:
        front = np.array(outlet["cornersMm"])
        rear = front - 28 * np.array(outlet["inwardNormal"])
        parts.append(dict(name=f"Fan pair {outlet['pair']} housing envelope", vertices=np.vstack([front, rear]).tolist(), faces=faces))
    return parts


def surface_points(part):
    v = np.array(part["v"])
    tri = v[part["f"]]
    pts = [v, tri.mean(1), ((tri + np.roll(tri, 1, axis=1)) / 2).reshape(-1, 3)]
    # Denser: points across each triangle (the proxies are large boxes and cylinders).
    for u, w in ((0.25, 0.25), (0.5, 0.25), (0.25, 0.5)):
        pts.append(tri[:, 0] * (1 - u - w) + tri[:, 1] * u + tri[:, 2] * w)
    return np.vstack(pts)


def material_clearance(bodies, xyz, along, printing, width, recent, stride=1, limit=40.0):
    """Each body's smallest distance to the printed bead's surface over the poses, and where."""
    # The material: the printed path (the sampled tip positions while printing), every 2 mm.
    pts, s = [], []
    for i in range(1, len(xyz)):
        if not printing[i]:
            continue
        seg = np.linalg.norm(xyz[i] - xyz[i - 1])
        n = max(1, int(math.ceil(seg / 2.0)))
        for k in range(1, n + 1):
            f = k / n
            pts.append(xyz[i - 1] * (1 - f) + xyz[i] * f)
            s.append(along[i - 1] * (1 - f) + along[i] * f)
    if not pts:
        return []
    mat = np.array(pts)
    mat_s = np.array(s)
    cell = 25.0
    keys = np.floor(mat / cell).astype(np.int64)
    grid = {}
    for idx, key in enumerate(map(tuple, keys)):
        grid.setdefault(key, []).append(idx)
    grid = {k: np.array(v) for k, v in grid.items()}
    out = []
    for b in bodies:
        local = surface_points(b)
        best = (np.inf, -1)
        below = 0
        for i in range(0, len(xyz), stride):
            if along[i] <= recent:
                continue
            world = local @ b["r"][i].T + b["t"][i]
            lo = np.floor((world.min(0) - limit) / cell).astype(int)
            hi = np.floor((world.max(0) + limit) / cell).astype(int)
            cand = [grid[(x, y, z)] for x in range(lo[0], hi[0] + 1) for y in range(lo[1], hi[1] + 1)
                    for z in range(lo[2], hi[2] + 1) if (x, y, z) in grid]
            if not cand:
                continue
            idx = np.concatenate(cand)
            idx = idx[mat_s[idx] < along[i] - recent]
            if not len(idx):
                continue
            m = mat[idx]
            d = np.sqrt(((world[:, None, :] - m[None, :, :]) ** 2).sum(-1)).min() - width / 2
            if d < 5.0:
                below += 1
            if d < best[0]:
                best = (d, i)
        if best[1] >= 0:
            out.append(dict(part=b["part"]["name"], minClearanceMm=round(float(best[0]), 2), sample=best[1], posesBelow5mm=below))
    out.sort(key=lambda r: r["minClearanceMm"])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("src", type=Path, help="the program's .SRC")
    ap.add_argument("--handover", type=Path, default=HANDOVER)
    ap.add_argument("--base", type=float, nargs=3, default=[0.0, 50.0, -0.3], help="TR_BASE X Y Z (no rotation)")
    ap.add_argument("--width", type=float, default=5.0, help="bead width (mm)")
    ap.add_argument("--recent", type=float, default=20.0, help="the bead being laid: this much path behind the nozzle is left out (mm)")
    ap.add_argument("--stride", type=int, default=1, help="check the material on every Nth sample only (faster)")
    ap.add_argument("--output", type=Path)
    args = ap.parse_args()

    kin, proxy, cad, qa, fans = load_handover(args.handover)
    poses = parse(args.src.read_text())
    xyz, rotation, lines, printing, along = samples(poses, np.array(args.base))
    report = dict(program=str(args.src), targets=len(poses), samples=len(xyz))

    # 1. Reach and joint limits.
    try:
        joints = kin.inverse(xyz, rotation)
    except AssertionError as e:
        report["reach"] = f"out of reach: {e}"
        print(json.dumps(report, indent=2))
        sys.exit(1)
    margin = np.minimum(joints[:, :5] - kin.LIMITS[:5, 0], kin.LIMITS[:5, 1] - joints[:, :5])
    report["jointLimitViolations"] = int(np.any(margin < 0, axis=1).sum())
    report["minimumAxisMarginsDeg"] = margin.min(axis=0).round(2).tolist()
    report["axisRangesDeg"] = [[round(float(joints[:, k].min()), 1), round(float(joints[:, k].max()), 1)] for k in range(6)]
    frames = [kin.forward(joints, n) for n in range(7)]

    # 2. Tool, arm and pedestal.
    excluded = set(qa["collisionExcludedToolParts"])
    tool = [proxy.body(p, frames, rotation, xyz) for p in [*cad["toolShapes"], *fan_parts(fans)] if p["name"] not in excluded]
    arm = [proxy.body(p, frames, rotation, xyz) for p in cad["robotShapes"] if p.get("collisionRole") != "pedestal"]
    pedestals = [proxy.body(p, frames, rotation, xyz) for p in cad["robotShapes"] if p.get("collisionRole") == "pedestal"]

    def hits(pairs):
        out = []
        for a, b in pairs:
            ids = proxy.intersect(a, b)
            if len(ids):
                out.append(dict(parts=[a["part"]["name"], b["part"]["name"]], poses=len(ids), firstLine=int(lines[ids].min()), lastLine=int(lines[ids].max())))
        return out

    report["toolVsArm"] = hits((a, b) for a in tool for b in arm if b["part"]["frame"] != 6)
    report["armSelf"] = hits((a, b) for i, a in enumerate(arm) for b in arm[i + 1:] if abs(a["part"]["frame"] - b["part"]["frame"]) > 1)
    report["pedestal"] = hits((p, ped) for ped in pedestals for p in [*tool, *(b for b in arm if b["part"]["frame"] > 0)])

    # 3. The bed.
    bed_z = qa["bedBoundsWorldMm"]["z"]
    target = qa["bedClearanceTargetMm"]
    bed = []
    for b in tool + arm:
        clear = b["lo"][:, 2] - bed_z
        n = int(clear.argmin())
        bed.append(dict(part=b["part"]["name"], minMm=round(float(clear[n]), 2), line=int(lines[n]),
                        posesBelowTarget=int((clear < target - 1e-4).sum())))
    bed.sort(key=lambda r: r["minMm"])
    report["bedClearanceTargetMm"] = target
    report["bedClosest"] = bed[:6]
    report["bedViolationsExcludingNozzle"] = [r for r in bed if r["part"] != "Nozzle" and r["posesBelowTarget"]]

    # 4. The printed material.
    near = [b for b in tool if b["part"]["name"] != "Nozzle"] + [b for b in arm if b["part"]["frame"] >= 4]
    mat = material_clearance(near, xyz, along, printing, args.width, args.recent, max(1, args.stride))
    for r in mat:
        i = r.pop("sample")
        r["line"] = int(lines[i])
        r["tcpWorld"] = xyz[i].round(1).tolist()
        r["tiltDeg"] = round(float(np.degrees(np.arccos(np.clip(rotation[i, 2, 2], -1, 1)))), 1)
    report["materialStride"] = max(1, args.stride)
    report["materialClearanceTargetMm (fans)"] = 5.0
    report["materialClearances"] = mat
    report["materialCollisions"] = [r for r in mat if r["minClearanceMm"] < 0]
    report["maxTiltDeg"] = round(float(np.degrees(np.arccos(np.clip(rotation[:, 2, 2], -1, 1))).max()), 2)

    ok = (report["jointLimitViolations"] == 0 and not report["toolVsArm"] and not report["armSelf"]
          and not report["pedestal"] and not report["bedViolationsExcludingNozzle"] and not report["materialCollisions"])
    fans_ok = all(r["minClearanceMm"] >= 5.0 for r in mat if "fan" in r["part"].lower())
    report["verdict"] = ("no modelled collisions or limit violations" if ok else "PROBLEMS: see above") + ("" if fans_ok else "; a fan comes within 5 mm of the print")
    text = json.dumps(report, indent=2)
    if args.output:
        args.output.write_text(text + "\n")
    print(text)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
