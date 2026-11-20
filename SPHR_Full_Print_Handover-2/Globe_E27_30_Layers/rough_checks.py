"""Quick checks on the nominal KRL replay. Run after build_trumpet.py."""
import argparse
from pathlib import Path
import base64
import csv
import gzip
import json
import math
import re

import numpy as np
from kinematics import LIMITS, forward, inverse
from trumpet_profile import profile_at_height

ROOT = Path(__file__).resolve().parent
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output',type=Path,default=ROOT)
output=parser.parse_args().output
qa = json.loads((output/'QA.json').read_text())
cad = json.loads((ROOT/'cad_proxy.json').read_text())
html = (output/'Non_Planar_Demo.html').read_text(encoding='utf-8')
packed = re.search(r'(?<![A-Za-z0-9+/])[A-Za-z0-9+/]{100000,}={0,2}', html)[0]
rows = np.frombuffer(gzip.decompress(base64.b64decode(packed)), '<f4').reshape(-1, 25)
# Check actual target poses plus three slerped samples per moving segment.
movement = (np.linalg.norm(np.diff(rows[:, :3], axis=0), axis=1) > 0) | (
    np.abs(np.sum(rows[:-1,-4:]*rows[1:,-4:],axis=1)) < 0.999999)
start, end = rows[:-1][movement], rows[1:][movement]
q0, q1 = start[:, -4:].astype(float), end[:, -4:].astype(float)
q0 /= np.linalg.norm(q0,axis=1)[:,None]
q1 /= np.linalg.norm(q1,axis=1)[:,None]
dot = np.sum(q0*q1,axis=1)
q1 = np.where(dot[:,None]<0,-q1,q1)
arc = np.arccos(np.clip(abs(dot),0,1))
sine = np.sin(arc)
fractions = (.25,.5,.75)
interior_xyz = np.concatenate([(1-f)*start[:,:3]+f*end[:,:3] for f in fractions])
interior_ref = np.concatenate([(1-f)*start[:,-10:-4]+f*end[:,-10:-4] for f in fractions])
qs = []
for f in fractions:
    a = np.divide(np.sin((1-f)*arc),sine,out=np.full_like(sine,1-f),where=sine>1e-6)
    b = np.divide(np.sin(f*arc),sine,out=np.full_like(sine,f),where=sine>1e-6)
    qs.append(a[:,None]*q0+b[:,None]*q1)
xyz = np.vstack([rows[:, :3],interior_xyz])
refs = np.r_[rows[:, 10],np.tile(end[:,10],len(fractions))].astype(int)
q = np.vstack([rows[:, -4:], *qs]).astype(float)
w,x,y,z = (q / np.linalg.norm(q, axis=1)[:,None]).T
rotation = np.column_stack([
    1-2*(y*y+z*z), 2*(x*y-w*z), 2*(x*z+w*y),
    2*(x*y+w*z), 1-2*(x*x+z*z), 2*(y*z-w*x),
    2*(x*z-w*y), 2*(y*z+w*x), 1-2*(x*x+y*y),
]).reshape(-1,3,3)
# Keep the sampled IK on the same wrap as the programmed replay.
interior_joints = inverse(interior_xyz,rotation[len(rows):])
interior_joints += 360*np.round((interior_ref-interior_joints)/360)
joints = np.vstack([rows[:, -10:-4],interior_joints])
frames = [forward(joints, n) for n in range(7)]

from proxy_collision import body, intersect

outlets=json.loads((ROOT/'fan_pair_coordinates.json').read_text())['providedPairs']
fan_parts=[]
faces=[[0,2,1],[0,3,2],[4,5,6],[4,6,7]]
for i in range(4):
    j=(i+1)%4
    faces.extend([[i,j,j+4],[i,j+4,i+4]])
for outlet in outlets:
    front=np.array(outlet['cornersMm'])
    rear=front-28*np.array(outlet['inwardNormal'])
    fan_parts.append(dict(name=f"Fan pair {outlet['pair']} housing envelope",
                          vertices=np.vstack([front,rear]),faces=faces))
excluded_tool_parts=set(qa['collisionExcludedToolParts'])
tool=[body(part, frames,rotation,xyz) for part in [*cad['toolShapes'],*fan_parts]
      if part['name'] not in excluded_tool_parts]
arm=[body(part, frames,rotation,xyz) for part in cad['robotShapes']
     if part.get('collisionRole')!='pedestal']
pedestals=[body(part, frames,rotation,xyz) for part in cad['robotShapes']
           if part.get('collisionRole')=='pedestal']

def check_pairs(pairs):
    hits = []
    for a,b in pairs:
        ids = intersect(a,b)
        if len(ids):
            lines = sorted(set(refs[ids]))
            hits.append(dict(parts=[a['part']['name'],b['part']['name']], poses=len(ids),
                             lineCount=len(lines), firstLine=int(lines[0]), lastLine=int(lines[-1]),
                             exampleTCP=xyz[ids[0]].round(2).tolist()))
    return hits

tool_arm = check_pairs((a,b) for a in tool for b in arm if b['part']['frame'] != 6)
arm_self = check_pairs((a,b) for i,a in enumerate(arm) for b in arm[i+1:]
                       if abs(a['part']['frame']-b['part']['frame']) > 1)
# The pedestal shares the static mounting plane. Check it against every
# movable link, including A1, and every enabled tool proxy.
pedestal_pairs=[(part,pedestal) for pedestal in pedestals
                for part in [*tool,*(b for b in arm if b['part']['frame']>0)]]
pedestal_collisions=check_pairs(pedestal_pairs)
pedestal_clearances=[]
for part,pedestal in pedestal_pairs:
    separation=np.maximum(0.,np.maximum(pedestal['lo']-part['hi'],part['lo']-pedestal['hi']))
    gaps=np.linalg.norm(separation,axis=1)
    j=int(gaps.argmin())
    pedestal_clearances.append(dict(part=part['part']['name'],pedestal=pedestal['part']['name'],
                                    lowerBoundMm=round(float(gaps[j]),3),line=int(refs[j])))
pedestal_clearances.sort(key=lambda part:part['lowerBoundMm'])
bed_bounds = qa['bedBoundsWorldMm']
bed_surface = bed_bounds['z']
bed = []
for b in tool+arm:
    clear = b['lo'][:,2]-bed_surface
    n = int(clear.argmin())
    bed.append(dict(part=b['part']['name'], marginMm=round(float(clear[n]),2), line=int(refs[n])))
bed.sort(key=lambda p:p['marginMm'])
# The lower bound accounts for the finite bed rectangle. A nozzle tip at
# the brim is intentional; check all other modelled extruder parts separately.
minimum_bed_clearance = 10.
bed_tool = []
for b in tool:
    dx=np.maximum(0.,np.maximum(bed_bounds['xMin']-b['hi'][:,0],b['lo'][:,0]-bed_bounds['xMax']))
    dy=np.maximum(0.,np.maximum(bed_bounds['yMin']-b['hi'][:,1],b['lo'][:,1]-bed_bounds['yMax']))
    dz=b['lo'][:,2]-bed_surface
    gap=np.where((dx==0)&(dy==0)&(dz<0),dz,np.sqrt(dx*dx+dy*dy+np.maximum(0,dz)**2))
    at=int(gap.argmin())
    bed_tool.append(dict(part=b['part']['name'],minMm=round(float(gap[at]),4),line=int(refs[at]),
                         posesBelowTarget=int(np.sum(gap < minimum_bed_clearance-1e-4))))
bed_tool.sort(key=lambda item:item['minMm'])
non_nozzle_bed_violations=[p for p in bed_tool if p['part']!='Nozzle' and p['posesBelowTarget']]


# Point samples on the proxy surfaces against a solid annular envelope for the
# previously printed wall and brim. The wall envelope below the live bead
# deliberately omits the newest bead and does not model sag or displacement.
src = (output/'SPHR.SRC').read_text().splitlines()
start_poses = start
taper_tag=';TOP TAPER' if ';TOP TAPER' in src else ';END PRINT'
start, wall, taper, lift = (src.index(tag)+1 for tag in (';BRIM',';LAYER 1',taper_tag,';LIFT'))
wall_ids = (refs > wall) & (refs < taper)
after_wall = refs >= taper
brim_done = refs >= wall
centre = np.array(qa['printCentreWorldMm'])
stem = qa['initialDiameterMm']/2
lip = qa['maximumRadiusMm']
width = qa['nominalBeadWidthMm']
part_hits = []
for b in tool+arm:
    if b['part']['name'] == 'Nozzle':
        continue
    tri = b['v'][b['f']]
    points = np.vstack((b['v'], tri.mean(1), ((tri+np.roll(tri,1,axis=1))/2).reshape(-1,3)))
    close = np.where(np.all(b['lo'] <= centre + [lip+width,lip+width,qa["heightMm"]+1],axis=1) &
                     np.all(b['hi'] >= centre + [-lip-width,-lip-width,-1],axis=1))[0]
    hits = []
    for ids in np.array_split(close, max(1,math.ceil(len(close)/512))):
        loc = points @ b['r'][ids].transpose(0,2,1) + (b['t'][ids]-centre)[:,None,:]
        r = np.linalg.norm(loc[:,:,:2],axis=-1)
        h = loc[:,:,2]
        radius,_ = profile_at_height(h-qa['wallZOffsetMm'])
        cap = np.where(after_wall[ids],qa['heightMm']+qa['wallZOffsetMm'],np.where(wall_ids[ids],xyz[ids,2]-centre[2]-0.3,0.))
        stem_filled = (h > 1) & (h <= cap[:,None]) & (np.abs(r-radius)<=width/2)
        brim_filled = brim_done[ids,None] & (np.abs(h)<=1) & (r>=stem-width/2) & (r<=stem+qa['brimWidthMm']+width/2)
        hits.extend(ids[np.any(stem_filled|brim_filled,axis=1)].tolist())
    if hits:
        part_hits.append(dict(part=b['part']['name'], poses=len(hits),
                              firstLine=int(min(refs[hits])), lastLine=int(max(refs[hits]))))

# Nominal revisit time from KRL deposition segments (not a controller clock).
gcode = (output/'Non_Planar_Demo.gcode').read_text().splitlines()
path = np.array([tuple(map(float,re.search(r'X(\S+) Y(\S+) Z(\S+)',line).groups()))
                 for line in gcode if line.startswith(('G0 ','G1 '))])
brim_end = qa['brimMotions']
brim_path = path[:brim_end+1, :2] + centre[:2]
brim_envelope = dict(xMin=float(brim_path[:,0].min()-width/2),
                     xMax=float(brim_path[:,0].max()+width/2),
                     yMin=float(brim_path[:,1].min()-width/2),
                     yMax=float(brim_path[:,1].max()+width/2))
overhang = dict(
    xMin=max(0., bed_bounds['xMin']-brim_envelope['xMin']),
    xMax=max(0., brim_envelope['xMax']-bed_bounds['xMax']),
    yMin=max(0., bed_bounds['yMin']-brim_envelope['yMin']),
    yMax=None if bed_bounds['yMax'] is None else max(0., brim_envelope['yMax']-bed_bounds['yMax']),
)

turn_steps = qa['taperMotions']
bed_y_edges = [bed_bounds['yMin'], *bed_bounds['sectionSeamsY'], bed_bounds['yMax']]
brim_sections = [[lo,hi] for lo,hi in zip(bed_y_edges,bed_y_edges[1:])
                 if brim_envelope['yMin'] <= hi and brim_envelope['yMax'] >= lo]
report=dict(
    excludedToolParts=sorted(excluded_tool_parts),
    samples=len(xyz), movingSegments=len(end), intermediateFractions=list(fractions),
    toolVsRobot=tool_arm, nonadjacentRobotLinks=arm_self,
    pedestalBoundsWorldMm=[dict(name=part['part']['name'],
                                minimum=part['lo'][0].tolist(),maximum=part['hi'][0].tolist())
                          for part in pedestals],
    pedestalCollisions=pedestal_collisions, pedestalClearances=pedestal_clearances,
    bedSurfaceZMm=bed_surface, closestBed=bed[:5],
    toolBedClearanceTargetMm=minimum_bed_clearance, toolBedClearances=bed_tool,
    toolBedClearanceViolationsExcludingNozzle=non_nozzle_bed_violations,
    bedBoundsWorldMm=bed_bounds, brimBeadEnvelopeWorldMm=brim_envelope,
    brimOverhangMm=overhang, brimBedSectionYRangesMm=brim_sections,
    printedPartEnvelope=part_hits,
    exclusions=['Initial PTP, cell fixtures, cables and controller blending',
                'Intended flange attachment and adjacent robot link pairs',
                'Nozzle contact with print, newest turn and brim before completion in part envelope check'])
(output/'collision_report.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))

# Check sampled surfaces against the wall already deposited at each pose.
# Round the wall's height upward by at most 0.5 mm and fill the whole brim
# from the first brim move. Both envelopes overestimate deposited material.
import shapely
from shapely.geometry import LineString, Point

height_bins = np.minimum(qa['heightMm'], np.ceil(2*np.maximum(
    0., xyz[:,2]-centre[2]-qa['wallZOffsetMm']))/2)
height_bins[after_wall] = qa['heightMm']
wall_groups = []
for upper in np.unique(height_bins[wall_ids | after_wall]):
    heights = np.linspace(0,upper,max(2,math.ceil(4*upper)+1))
    radii,_ = profile_at_height(heights)
    profile = np.column_stack([radii,heights+qa['wallZOffsetMm']])
    bead = (LineString(profile) if upper > 0 else Point(profile[0])).buffer(width/2)
    ids = np.flatnonzero((height_bins == upper) & (wall_ids | after_wall))
    wall_groups.append((bead, ids))

fan_clearance = []
finish = refs >= src.index(';END PRINT')+1
for b in tool:
    if not any(word in b['part']['name'].lower() for word in ('fan','blower','duct')):
        continue
    tri = b['v'][b['f']]
    points = np.vstack([b['v'],tri.mean(axis=1),((tri+np.roll(tri,1,axis=1))/2).reshape(-1,3)])
    min_brim = np.full(len(xyz),np.inf)
    min_wall = np.full(len(xyz),np.inf)
    for ids in np.array_split(np.flatnonzero(refs >= start), math.ceil(len(xyz)/256)):
        local = points @ b['r'][ids].transpose(0,2,1)+(b['t'][ids]-centre)[:,None,:]
        radii = np.linalg.norm(local[:,:,:2],axis=2)
        dr = np.maximum(0.,np.maximum(stem-width/2-radii,radii-(stem+qa['brimWidthMm']+width/2)))
        dz = np.maximum(0.,abs(local[:,:,2])-1.)
        min_brim[ids] = np.hypot(dr,dz).min(axis=1)
    # The 25 mm broad phase only discards distances proven above 25 mm.
    for bead, ids in wall_groups:
        lo_r,lo_z,hi_r,hi_z = bead.bounds
        ids = ids[(b['lo'][ids,2]-centre[2] <= hi_z+25) & (b['hi'][ids,2]-centre[2] >= lo_z-25)]
        for batch in np.array_split(ids, max(1,math.ceil(len(ids)/128))):
            if not len(batch):
                continue
            local = points @ b['r'][batch].transpose(0,2,1)+(b['t'][batch]-centre)[:,None,:]
            rr,zz = np.linalg.norm(local[:,:,:2],axis=2),local[:,:,2]
            near = (rr>=lo_r-25)&(rr<=hi_r+25)&(zz>=lo_z-25)&(zz<=hi_z+25)
            distances = np.full(rr.shape,25.)
            distances[near] = shapely.distance(shapely.points(rr[near],zz[near]),bead)
            min_wall[batch] = distances.min(axis=1)
    gap = np.minimum(min_brim,min_wall)
    worst = int(gap.argmin())
    bad = gap < 5.
    fan_clearance.append(dict(part=b['part']['name'],minSampledMaterialClearanceMm=round(float(gap[worst]),3),
                              finishMinimumSampledClearanceMm=round(float(gap[finish].min()),3),
                              sourceLine=int(refs[worst]),material='brim' if min_brim[worst] <= min_wall[worst] else 'wall',
                              posesBelow5mm=int(bad.sum()),violationLines=np.unique(refs[bad]).tolist()))
report['fanMaterialClearanceTargetMm'] = 5.
report['fanMaterialClearances'] = fan_clearance
joint_margin=np.minimum(joints[:,:5]-LIMITS[:5,0],LIMITS[:5,1]-joints[:,:5])
report['minimumSampledAxisMarginsDeg']=joint_margin.min(axis=0).tolist()
report['jointLimitViolations']=int(np.any(joint_margin<0,axis=1).sum())
report['finishSamples']=int(finish.sum())
report['finishMinimumSampledAxisMarginsDeg']=joint_margin[finish].min(axis=0).tolist()
report['maximumSampledTiltDeg'] = float(np.degrees(np.arccos(np.clip(rotation[:,2,2],-1,1))).max())
(output/'collision_report.json').write_text(json.dumps(report,indent=2))
print('Fan material clearance minimum',min(p['minSampledMaterialClearanceMm'] for p in fan_clearance))