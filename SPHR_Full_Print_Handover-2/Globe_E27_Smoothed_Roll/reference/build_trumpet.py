"""Build a synthetic extrusion example and replay its emitted KRL."""
from pathlib import Path
import argparse
import base64
import gzip
import hashlib
import json
import math
import re
from runpy import run_path
from types import SimpleNamespace
import sys
import zipfile

import numpy as np
import trumpet_profile
from kinematics import A, DD, AL, LIMITS, TOOL, inverse, forward, rot

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, default=ROOT)
parser.add_argument('--start-flow', type=float, default=.2)
parser.add_argument('--layer-2-flow', type=float, default=.8)
parser.add_argument('--brim-rings', type=int, default=13)
parser.add_argument('--brim-width', type=float, default=50.)
parser.add_argument('--last-brim-ring-offset', type=float, default=3.2)
parser.add_argument('--wall-start-height', type=float, default=.1)
parser.add_argument('--wall-speed', type=float, default=10.)
parser.add_argument('--profile', type=Path, help='Python profile definition for this variant')
args = parser.parse_args()
if not 0 < args.start_flow <= args.layer_2_flow <= 1:
    parser.error('Extrusion fractions must satisfy 0 < start flow <= layer 2 flow <= 1')
if args.brim_rings < 2:
    parser.error('At least two brim rings are needed to prescribe the inner and outer radii')
if not math.isfinite(args.wall_start_height) or args.wall_start_height <= 0:
    parser.error('Wall start height must be a positive distance in millimetres')
if not math.isfinite(args.wall_speed) or args.wall_speed <= 0:
    parser.error('Wall speed must be positive')
DEST = args.output.resolve()
DEST.mkdir(parents=True, exist_ok=True)
BASE = np.array([0., 50., -0.3])
BRIM_WIDTH = args.brim_width
profile = trumpet_profile if args.profile is None else SimpleNamespace(**run_path(str(args.profile)))
BEAD_WIDTH = profile.bead_width
STEM_RADIUS = profile.stem_radius
TARGET_CREST_ID = profile.target_crest_id
TARGET_OUTER_DIAMETER = 158.
LIP_RADIUS = (TARGET_OUTER_DIAMETER-BEAD_WIDTH)/2
BRIM_RINGS = args.brim_rings
BRIM_TRIM_MM = 10.
BRIM_RAMP_MM = 10.
BRIM_RAMP_STEPS = 5
FINAL_RING_GAP_MM = args.last_brim_ring_offset
if not (math.isfinite(BRIM_WIDTH) and 0 < FINAL_RING_GAP_MM < BRIM_WIDTH):
    parser.error('Brim width must exceed the positive innermost ring offset')
SEAM_ANGLE = math.pi/2  # +Y in world coordinates.
BED_Z_BELOW_BRIM = 1.
WALL_START_ABOVE_BED = args.wall_start_height
WALL_Z_OFFSET = WALL_START_ABOVE_BED-BED_Z_BELOW_BRIM
START_RAMP_INITIAL_FRACTION = args.start_flow
START_RAMP_SECTORS = 12
LAYER_2_SCREW_FRACTION = args.layer_2_flow
LAYER_RAMP_INTERVALS = 7
BRIM_SPEED = 10.
WALL_SPEED = args.wall_speed
SCREW_PER_MM_S = 100.
WALL_FLOW_MULTIPLIER = 3.
PURGE_SECONDS = 20
PURGE_TRANSFER_SPEED = 100.
BRIM_APPROACH_SPEED = 20.

def screw_for(speed, multiplier=1.):
    return round(SCREW_PER_MM_S*speed*multiplier)

def wall_screw(speed):
    return screw_for(speed,WALL_FLOW_MULTIPLIER)

TAPER_SECTORS = 12

def write_krl(name, lines):
    (DEST / name).write_bytes(('\r\n'.join(lines) + '\r\n').encode('ascii'))

def send(payload):
    return f'RET = EKI_SEND("EthernetKRL_Config", "{payload}#")'

def lin(x, y, z, abc, blend=True):
    x,y,z = np.array([x,y,z]) - BASE
    a,b,c=abc
    return (f'LIN {{X {x:.2f},Y {y:.2f},Z {z:.2f},A {a:.2f},B {b:.2f},C {c:.2f}}}'
            + (' C_DIS' if blend else ''))

cx, cy = 1088.304936, -115.143158
MEASURED_BED_Z = -659.
bed = MEASURED_BED_Z+BED_Z_BELOW_BRIM
height, flare_start, pitch = profile.height, profile.flare_start, profile.pitch
layer_count, profile_length_mm = profile.layer_count, profile.profile_length_mm
profile_at_distance, profile_at_height = profile.profile_at_distance, profile.profile_at_height
fillet_radius_mm, lip_angle_deg = profile.fillet_radius_mm, profile.lip_angle_deg

steps = 60
flare_height = height-flare_start
MAX_NOZZLE_TILT_DEG = 45.
with np.load(ROOT/"layer_tilt.npz") as tilt_plan:
    spiral_tilts = tilt_plan["spiral"]
    taper_tilt = float(tilt_plan["taper"])

def flare_geometry(z):
    return profile_at_height(z)

def radius(z):
    return profile_at_height(z)[0]

def surface_pose(h, theta, degrees):
    # +Z points from the nozzle into the tool.
    derivative=math.tan(math.radians(degrees))
    co,si=math.cos(theta),math.sin(theta)
    z=np.array([derivative*co,derivative*si,1.])
    z/=np.linalg.norm(z)
    # Project world -Y into the plane perpendicular to the nozzle.
    x=np.array([0.,-1.,0.])
    x-=z*(z@x)
    x/=np.linalg.norm(x)
    y=np.cross(z,x)
    r=np.column_stack([x,y,z])
    return np.degrees([math.atan2(r[1,0],r[0,0]),
        math.atan2(-r[2,0],math.hypot(r[0,0],r[1,0])),math.atan2(r[2,1],r[2,2])])

spiral = []
spiral_profile = []
spiral_steps=math.ceil(profile_length_mm/pitch*steps)
for i in range(spiral_steps+1):
    r,h,tilt = profile_at_distance(min(i*pitch/steps,profile_length_mm))
    theta = SEAM_ANGLE+math.tau*i/steps
    spiral.append((r*math.cos(theta),r*math.sin(theta),h+WALL_Z_OFFSET))
    spiral_profile.append((r,h,tilt))

# Omit the first 10 mm of arc after +Y on each ring. The ring ends
# at +Y and links diagonally to the following ring's trimmed start.
brim = []
ring_ranges = []
brim_ramp_by_point = {}
brim_screw = screw_for(BRIM_SPEED)
wall_screw_full = wall_screw(WALL_SPEED)
spiral_start_screw = round(wall_screw_full*START_RAMP_INITIAL_FRACTION)
layer_2_screw = round(wall_screw_full*LAYER_2_SCREW_FRACTION)
assert steps % START_RAMP_SECTORS == 0
first_turn_screw_commands = [
    round(spiral_start_screw+(layer_2_screw-spiral_start_screw)*step/START_RAMP_SECTORS)
    for step in range(START_RAMP_SECTORS+1)
]
first_turn_screw_by_move = {
    step*(steps//START_RAMP_SECTORS):command
    for step,command in enumerate(first_turn_screw_commands[:-1])
}
layer_screw_commands = {
    2+step:round(layer_2_screw+(wall_screw_full-layer_2_screw)*step/LAYER_RAMP_INTERVALS)
    for step in range(LAYER_RAMP_INTERVALS+1)
}
layer_screw_by_move = {(layer-1)*steps:command for layer,command in layer_screw_commands.items()}
wall_screw_by_move = {**first_turn_screw_by_move, **layer_screw_by_move}
layer_flow_routines = {command:f'SPHR_FLOW_{command}' for command in layer_screw_commands.values()}
layer_flow_triggers = {
    command:f'TRIGGER WHEN DISTANCE=0 DELAY=0 DO {routine}() PRIO=-1'
    for command,routine in layer_flow_routines.items()
}
radii = np.linspace(STEM_RADIUS+BRIM_WIDTH,STEM_RADIUS+FINAL_RING_GAP_MM,BRIM_RINGS)
for ring,r in enumerate(radii,1):
    begin=len(brim)
    sweep=math.tau-BRIM_TRIM_MM/r
    segments=math.ceil(steps*sweep/math.tau)
    first,last=SEAM_ANGLE+BRIM_TRIM_MM/r,SEAM_ANGLE+math.tau
    if ring == BRIM_RINGS:
        # Insert the exact ramp boundary and five equal arc increments.
        ramp_start=last-BRIM_RAMP_MM/r
        before_ramp=math.ceil(steps*(ramp_start-first)/math.tau)
        for theta in np.linspace(first,ramp_start,before_ramp+1):
            brim.append((r*math.cos(theta),r*math.sin(theta),0.))
        for step,theta in enumerate(np.linspace(ramp_start,last,BRIM_RAMP_STEPS+1)[1:],1):
            fraction=step/BRIM_RAMP_STEPS
            command=round(brim_screw+(spiral_start_screw-brim_screw)*fraction)
            brim_ramp_by_point[len(brim)]=command
            brim.append((r*math.cos(theta),r*math.sin(theta),0.))
    else:
        for theta in np.linspace(first,last,segments+1):
            brim.append((r*math.cos(theta),r*math.sin(theta),0.))
    ring_ranges.append((begin,len(brim)-1))
end_theta=SEAM_ANGLE+math.tau*spiral_steps/steps
taper = [(radius(height)*math.cos(end_theta+math.tau*i/steps),
          radius(height)*math.sin(end_theta+math.tau*i/steps),height+WALL_Z_OFFSET)
         for i in range(1,steps+1)]
warmup = (cx,cy+250,-590.)  # Retain the normal world-space purge point.
print_start = (cx+brim[0][0],cy+brim[0][1],bed)
start_pose = surface_pose(0,SEAM_ANGLE,spiral_tilts[0])
stride = steps//TAPER_SECTORS
assert steps % TAPER_SECTORS == 0
gcode = ['; Trumpet trial bead centreline, mm','G21','G90','M83',
         f'G0 X{brim[0][0]:.2f} Y{brim[0][1]:.2f} Z0.00 F{BRIM_SPEED*60:.0f}']
previous = brim[0]

def deposit(point, fraction=1., speed=WALL_SPEED):
    global previous
    x,y,z = point
    e = math.dist(previous,point)*0.5*fraction
    gcode.append(f'G1 X{x:.2f} Y{y:.2f} Z{z:.2f} E{e:.5f} F{speed*60:.0f}')
    previous = point

gcode.append(';BRIM')
for ring,(begin,end) in enumerate(ring_ranges,1):
    gcode.append(f';BRIM RING {ring}')
    for point_index in range(max(1,begin),end+1):
        command=brim_ramp_by_point.get(point_index)
        if command is not None:
            gcode.append(f';BRIM RAMP {command}')
        fraction=1. if command is None else command/brim_screw
        deposit(brim[point_index],fraction,speed=BRIM_SPEED)
gcode.append(';BRIM JOIN')
# Reduce flow and descend on the final deposited lead-in, with no travel.
deposit(spiral[0],WALL_FLOW_MULTIPLIER*spiral_start_screw/wall_screw_full,WALL_SPEED)
wall_command = spiral_start_screw
for i,point in enumerate(spiral[1:]):
    if i % steps == 0:
        gcode.append(f';LAYER {i//steps+1}')
    wall_command = wall_screw_by_move.get(i,wall_command)
    deposit(point,WALL_FLOW_MULTIPLIER*wall_command/wall_screw_full,WALL_SPEED)
gcode.append(';TOP TAPER')
for i,point in enumerate(taper):
    deposit(point,WALL_FLOW_MULTIPLIER*(1-(i//stride)/TAPER_SECTORS),WALL_SPEED)
(DEST/'Non_Planar_Demo.gcode').write_text('\n'.join(gcode)+'\n',encoding='ascii')

header = ['&ACCESS RVP', 'DEF SPHR()', 'DECL EKI_STATUS RET']
write_krl('SPHR.DAT', ['&ACCESS RVP', 'DEFDAT SPHR PUBLIC',
    'DECL FRAME TR_TOOL={X -508.00,Y 3.50,Z 139.34,A -180.00,B -90.00,C 0.00}',
    'DECL FRAME TR_BASE={X 0.00,Y 50.00,Z -0.30,A 0.00,B 0.00,C 0.00}',
    '; Estimated uniform-density volume centroid, relative to flange.',
    'DECL REAL TR_MASS=18.0',
    'DECL FRAME TR_COM={X -90.65,Y 5.95,Z 164.92,A 0.00,B 0.00,C 0.00}',
    'ENDDAT'])
lines = header + ['BAS (#INITMOV,0)', '$ACC.ORI1 = 150.0', '$ACC.ORI2 = 150.0', '$VEL.ORI1 = 200.0', '$VEL.ORI2 = 200.0', '$ADVANCE = 3', send('STOP_EXTRUDE'),
    '$TOOL = TR_TOOL',
    '; Reuse tool 6 inertia and inertia-axis orientation; override mass and COM XYZ.',
    '$LOAD = LOAD_DATA[6]', '$LOAD.M = TR_MASS',
    '$LOAD.CM.X = TR_COM.X', '$LOAD.CM.Y = TR_COM.Y', '$LOAD.CM.Z = TR_COM.Z',
    '$BASE = TR_BASE', '$IPO_MODE = #BASE', '$ORI_TYPE = #VAR',
    '; SIMULATION ONLY - verify cell, cables and controller.',
    'PTP $AXIS_ACT', send('SET_TEMP T0 220'), send('SET_TEMP T1 190'), send('SET_TEMP T2 70'),
    send('SET_FAN1 0'), send('SET_FAN2 5'), 'HALT', '$VEL.CP = 0.05',
    lin(*warmup,start_pose,blend=False), send('CHANGE_SPEED1 12000'), send('START_EXTRUDE'),
    f'WAIT SEC {PURGE_SECONDS}', send(f'CHANGE_SPEED1 {brim_screw}'),
    f'$VEL.CP = {PURGE_TRANSFER_SPEED/1000:g}',
    lin(print_start[0], print_start[1], bed+5,start_pose,blend=False),
    f'$VEL.CP = {BRIM_APPROACH_SPEED/1000:g}', lin(*print_start,start_pose,blend=False),
    '$APO.CDIS = 0.15', f'$VEL.CP = {BRIM_SPEED/1000:g}', ';BRIM']

# Re-read the generated source instead of sending the analytic points directly to KRL.
stage = 'brim'
brim_moves = spiral_moves = taper_moves = 0
for raw in (DEST/'Non_Planar_Demo.gcode').read_text().splitlines():
    if raw.startswith(';BRIM RING'):
        lines.append(raw)
    if raw.startswith(';BRIM RAMP '):
        lines.extend([raw,send(f'CHANGE_SPEED1 {int(raw.split()[-1])}')])
    if raw == ';BRIM JOIN':
        stage='join'
        lines.extend([raw,f'$VEL.CP = {WALL_SPEED/1000:g}'])
    if raw.startswith(';LAYER'):
        stage = 'wall'
        lines.append(raw)
    if raw == ';TOP TAPER':
        stage = 'taper'
        lines.append(raw)
    if not raw.startswith('G1 '):
        continue
    x,y,z,*_ = map(float,re.findall(r'[XYZEF](-?[\d.]+)',raw))
    pose = start_pose
    blend = True
    if stage in ('brim','join'):
        brim_moves += 1
        blend = stage == 'brim'
    elif stage == 'wall':
        spiral_moves += 1
        # Synchronize layer-boundary speed changes with the following LIN.
        completed = spiral_moves-1
        if completed in layer_screw_by_move:
            lines.append(layer_flow_triggers[layer_screw_by_move[completed]])
        elif completed and completed in first_turn_screw_by_move:
            lines.append(send(f'CHANGE_SPEED1 {first_turn_screw_by_move[completed]}'))
        if spiral_moves == 1:
            lines.append(send('SET_FAN1 255'))
        pose = surface_pose(spiral_profile[spiral_moves][1],SEAM_ANGLE+math.tau*spiral_moves/steps,spiral_tilts[spiral_moves])
    else:
        if taper_moves and taper_moves % stride == 0:
            fraction = 1-(taper_moves//stride)/TAPER_SECTORS
            lines.append(send(f'CHANGE_SPEED1 {round(wall_screw_full*fraction)}'))
        taper_moves += 1
        pose = surface_pose(height,end_theta+math.tau*taper_moves/steps,taper_tilt)
        blend = taper_moves < steps
    lines.append(lin(cx+x,cy+y,bed+z,pose,blend))
assert (brim_moves,spiral_moves,taper_moves) == (len(brim),len(spiral)-1,steps)
taper_lines = [line for line in lines[lines.index(';TOP TAPER'):] if line.startswith('LIN ')]
assert len(taper_lines) == steps
lines += ['WAIT SEC 0',send('STOP_EXTRUDE'),send('CHANGE_SPEED1 0')]
lines += [send(f'SET_TEMP T{i} 0') for i in range(3)]
lines += [';WIPE',f'$VEL.CP = {WALL_SPEED/1000:g}',*taper_lines]
x,y,z,*top_pose = map(float,re.findall(r'[XYZABC] (-?[\d.]+)',taper_lines[-1]))
lift_target = np.array([x,y,z])+BASE+[0,0,50.]
lines += [';LIFT','$VEL.CP = 0.05',lin(*lift_target,top_pose,False),
          ';UPRIGHT','$VEL.ORI1 = 30.0','$VEL.ORI2 = 30.0',
          lin(*lift_target,(-90.,0.,0.),False),'END']
for command,routine in layer_flow_routines.items():
    lines += ['',f'DEF {routine}()','DECL EKI_STATUS RET',send(f'CHANGE_SPEED1 {command}'),'END']
write_krl('SPHR.SRC',lines)

# Replay the programmed subset in execution order. No generic KRL interpreter.
script = (DEST/'SPHR.SRC').read_text().splitlines()
state = speed = flow = fan1 = fan2 = layer = phase = 0
temps = [0,0,0]
xyz = None
now = 0.
rows = []
layer_ranges = [None] * (layer_count+1)
orientations=[]
pose=None
def row(line, velocity=None):
    if xyz is not None:
        rows.append([*xyz, now, speed if velocity is None else velocity,
                     flow, fan1, state, phase, layer, line, *temps, fan2])
        orientations.append(pose)

# Replay the DISTANCE=0 commands at their nominal waypoints.
layer_flow_calls = {
    trigger:script[script.index(f'DEF {layer_flow_routines[command]}()')+2]
    for command,trigger in layer_flow_triggers.items()
}
for n,s in enumerate(script,1):
    if s == 'END':
        break
    s = layer_flow_calls.get(s,s)
    if s.startswith(';LAYER'):
        layer = int(s.split()[-1])
        phase = 1
    elif s == ';BRIM':
        phase = 1
    elif s in (';WIPE',';LIFT',';UPRIGHT'):
        phase = 4
    elif s.startswith('$VEL.CP'):
        speed = float(s.split('=')[-1])*1000
    elif m := re.search(r'EKI_SEND\("[^"]+", "([^#]+)#"\)',s):
        cmd,*args = m[1].split()
        if cmd == 'STOP_EXTRUDE':
            state = 0
            phase = 2 if layer else 0
        elif cmd == 'START_EXTRUDE':
            state = 1
            phase = 1 if phase == 1 or layer else 3
        elif cmd=='CHANGE_SPEED1':
            flow=int(args[0])
        elif cmd=='SET_FAN1':
            fan1=int(args[0])
        elif cmd=='SET_FAN2':
            fan2=int(args[0])
        elif cmd=='SET_TEMP':
            temps[int(args[0][1])]=int(args[1])
        else:
            raise ValueError(f'Unsupported EKI payload on line {n}')
        row(n,0)
    elif s.startswith('WAIT SEC'):
        row(n,0)
        now += float(s.split()[-1])
        row(n,0)
    elif s.startswith('LIN '):
        x,y,z,*abc=map(float,re.findall(r'[XYZABC] (-?[\d.]+)',s))
        assert speed>0
        dest=tuple(np.array([x,y,z])+BASE)
        if xyz is not None:
            row(n)
            if phase==1:
                if layer_ranges[layer] is None:
                    layer_ranges[layer]=[now,now]
            distance = math.dist(dest,xyz)
            if distance:
                now += distance/speed
            else:
                delta = rot(abc) @ rot(pose).T
                angle = math.degrees(math.acos(np.clip((np.trace(delta)-1)/2,-1,1)))
                now += angle/30+.2 if angle else 0.
        xyz,pose=dest,abc
        row(n)
        if phase==1:
            layer_ranges[layer][1]=now
    elif not s.startswith((';','&','DEF ','DECL ','BAS ','$ADVANCE','$ACC','$VEL.ORI','$TOOL','$LOAD','$BASE','$IPO_MODE','$ORI_TYPE','$APO','PTP ','HALT','END')):
        raise ValueError(f'Unsupported KRL at line {n}: {s}')
assert all(layer_ranges[1:]) and (state,fan1,fan2,*temps)==(0,255,5,0,0,0)

data=np.array(rows)
rotations=np.array([rot(abc) for abc in orientations])
ha,hb,hc=(np.radians(np.array(orientations))/2).T
sa,sb,sc=np.sin([ha,hb,hc])
ca,cb,cc=np.cos([ha,hb,hc])
quaternions=np.column_stack([ca*cb*cc+sa*sb*sc, ca*cb*sc-sa*sb*cc,
    ca*sb*cc+sa*cb*sc, sa*cb*cc-ca*sb*sc])
joints=inverse(data[:,:3],rotations)
fk=forward(joints)@TOOL
error=float(np.linalg.norm(fk[:,:3,3]-data[:,:3],axis=1).max())
orientation_error=float(np.abs(fk[:,:3,:3]-rotations).max())
assert error<1e-6 and orientation_error<1e-8
midpoints=(data[:-1,:3]+data[1:,:3])/2
u,_,vh=np.linalg.svd((rotations[:-1]+rotations[1:])/2)
midj=inverse(midpoints,u@vh)
miderror=float(np.linalg.norm((forward(midj)@TOOL)[:,:3,3]-midpoints,axis=1).max())
assert miderror<1e-6
limits=LIMITS[:5]
tested=np.vstack([joints,midj])
margin=np.minimum(tested[:,:5]-limits[:,0],limits[:,1]-tested[:,:5])

moving=np.diff(data[:,3])>0
durations=np.diff(data[:,3])[moving]
sampled_positions=[]
sampled_rotations=[]
q0=quaternions[:-1][moving]
q1=quaternions[1:][moving]
dot=(q0*q1).sum(1)
q1=q1*np.where(dot<0,-1,1)[:,None]
omega=np.arccos(np.clip(np.abs(dot),0,1))
denom=np.sin(omega)
for fraction in np.linspace(0,1,9):
    sampled_positions.append(data[:-1,:3][moving]*(1-fraction)+data[1:,:3][moving]*fraction)
    w0=np.divide(np.sin((1-fraction)*omega),denom,out=np.full_like(denom,1-fraction),where=denom>1e-6)
    w1=np.divide(np.sin(fraction*omega),denom,out=np.full_like(denom,fraction),where=denom>1e-6)
    q=q0*w0[:,None]+q1*w1[:,None]
    w,x,y,z=(q/np.linalg.norm(q,axis=1)[:,None]).T
    sampled_rotations.append(np.column_stack([1-2*(y*y+z*z),2*(x*y-w*z),2*(x*z+w*y),
        2*(x*y+w*z),1-2*(x*x+z*z),2*(y*z-w*x),2*(x*z-w*y),2*(y*z+w*x),1-2*(x*x+y*y)]).reshape(-1,3,3))
subj=inverse(np.stack(sampled_positions,axis=1).reshape(-1,3),
    np.stack(sampled_rotations,axis=1).reshape(-1,3,3)).reshape(-1,9,6)
sampled_margins=np.minimum(subj[...,:5]-limits[:,0],limits[:,1]-subj[...,:5])
axis_peak=(np.abs(np.diff(subj,axis=1))*8/durations[:,None,None]).max(axis=(0,1))
peak_speed=float(axis_peak.max())

model=json.loads((ROOT/'cad_proxy.json').read_text())
outlets=json.loads((ROOT/'fan_pair_coordinates.json').read_text())['providedPairs']
bed_extra_tool_proxies=[]
for outlet in outlets:
    front=np.array(outlet['cornersMm'])
    back=front-28*np.array(outlet['inwardNormal'])
    bed_extra_tool_proxies.append(dict(name=f"Fan pair {outlet['pair']}",
                                       vertices=np.vstack([front,back]).tolist()))

report=dict(orientationAccelerationDegS2=[150.,150.], blendDistanceMm=.15, robot='KR50 R2500',pointCount=len(data),modelMotions=brim_moves+spiral_moves+taper_moves,
    brimMotions=brim_moves, spiralMotions=spiral_moves,taperMotions=taper_moves,
    collisionExcludedToolParts=["Heater barrel"],
    bedClearanceTargetMm=10., bedExtraToolProxies=bed_extra_tool_proxies,
    bedBoundsWorldMm=dict(xMin=713.304936, xMax=1913.304936, yMin=-1865.143158,
                          yMax=934.856842, z=float(print_start[2]-BED_Z_BELOW_BRIM),
                          sectionSeamsY=[-1165.143158,-465.143158,234.856842]),
    brimWidthMm=BRIM_WIDTH, brimRings=BRIM_RINGS, brimTrimArcMm=BRIM_TRIM_MM, brimTrimFrom="start",
    brimRadialPitchMm=(BRIM_WIDTH-FINAL_RING_GAP_MM)/(BRIM_RINGS-1),
    lastBrimRingToWallMm=FINAL_RING_GAP_MM, seamWorldDirection="+Y",
    wallStartAboveBedMm=WALL_START_ABOVE_BED, wallZOffsetMm=WALL_Z_OFFSET,
    brimRampArcMm=BRIM_RAMP_MM, brimRampScrewCommands=[brim_screw,*brim_ramp_by_point.values()],
    startRampScrewCommands=first_turn_screw_commands,
    startRampCompletedMoves=[*first_turn_screw_by_move,steps],
    layerStartScrewCommands=layer_screw_commands, startRampFullFlowLayer=2+LAYER_RAMP_INTERVALS,
    startRampPathLengthMm=sum(math.dist(a,b) for a,b in zip(spiral[:steps],spiral[1:steps+1])),
    brimSpeedMmS=BRIM_SPEED, brimScrewCommand=screw_for(BRIM_SPEED),
    printSpeedRangeMmS=[WALL_SPEED,WALL_SPEED], wallFlowMultiplier=WALL_FLOW_MULTIPLIER,
    purgeSeconds=PURGE_SECONDS, purgeToBrimSpeedMmS=PURGE_TRANSFER_SPEED,
    brimApproachSpeedMmS=BRIM_APPROACH_SPEED, firecapTemperatureC=70, firecapFanCommand=5,
    partFanAtSpiralStart=255,
    printScrewCommandRange=[wall_screw_full,wall_screw_full], baseOffsetMm=BASE.tolist(),
    taperSectors=TAPER_SECTORS, maximumNozzleTiltDeg=MAX_NOZZLE_TILT_DEG, tiltStrategy="Constant per turn with smooth seam transitions",
    nominalSeconds=float(np.float32(now)),bedZ=bed,layers=layer_count,layerRanges=layer_ranges,srcCount=1,
    purgeXYZ=warmup,wallConnections=[],a=A.tolist(),d=DD.tolist(),alpha=AL.tolist(),
    tool=TOOL.tolist(),printCentreWorldMm=[cx,cy,bed], heightMm=height,spiralPitchMm=pitch,
    pitchDefinition="arc length along the RZ wall profile", profileLengthMm=profile_length_mm,
    nominalOutsideLipDiameterMm=TARGET_OUTER_DIAMETER, straightThroatMm=flare_start,
    flareHeightMm=flare_height, initialFilletRadiusMm=fillet_radius_mm,
    jointLimitViolationVertices=int(np.any(margin<0,axis=1).sum()),
    jointLimitViolationSamples=int(np.any(sampled_margins<0,axis=-1).sum()),
    maxVertexPositionErrorMm=error,maxMidpointPositionErrorMm=miderror,
    minimumWristBendDeg=float(np.abs(tested[:,4]).min()),
    peakSampledAxisSpeedDegS=peak_speed, peakSampledSpeedPerAxisDegS=axis_peak.tolist(),
    orientationMatrixError=orientation_error,a6AngleLimitDeg=None, minimumJointMarginsDeg=margin.min(axis=0).tolist(),
    largestVertexJointStepDeg=float(np.abs(np.diff(joints,axis=0)).max()),
    cad=model,initialDiameterMm=2*STEM_RADIUS, nominalBeadWidthMm=BEAD_WIDTH,
    targetCrestIDMm=TARGET_CREST_ID, flareProfile="inferred sphere R95 with cubic throat blend", maximumRadiusMm=95., inferredDimensions=True, clearOpeningMm=150., inferredSphereRadiusMm=95.,
    lastTurnRadialStepMm=spiral_profile[-1][0]-spiral_profile[-steps-1][0],
    lastTurnRiseMm=height-spiral_profile[-steps-1][1],
    lipDiameterMm=round(2*radius(height),4),wallAngleFromVerticalDeg=lip_angle_deg,
    sourceHashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in
                  (DEST/'SPHR.SRC',DEST/'SPHR.DAT',DEST/'Non_Planar_Demo.gcode')})
(DEST/'QA.json').write_text(json.dumps({**report,'cad':{k:v for k,v in model.items()
    if k not in ('toolShapes','robotShapes')}},indent=2))
packed=base64.b64encode(gzip.compress(np.column_stack([data,joints,quaternions]).astype('<f4').tobytes())).decode()
template=(ROOT/'viewer_template.html').read_text(encoding='utf-8')
(DEST/'Non_Planar_Demo.html').write_text(template.replace('__LAYER_COUNT__',str(layer_count)).replace('__METADATA__',json.dumps(report,separators=(',',':')))
    .replace('__PACKED_DATA__',packed),encoding='utf-8')
print(f"Replay: {report['modelMotions']} print moves, {now:.2f} s, lip {2*radius(height):.2f} mm, FK error {error:.3g} mm, limits violations {report['jointLimitViolationVertices']}")

# Keep the controller import filenames stable on every rebuild.
archive = DEST/'SPHR.zip'
temporary_archive = archive.with_suffix('.zip.tmp')
with zipfile.ZipFile(temporary_archive, 'w', zipfile.ZIP_DEFLATED) as package:
    for name in ('SPHR.SRC', 'SPHR.DAT'):
        package.write(DEST/name, name)
temporary_archive.replace(archive)
