"""Build a 30-layer test or the full SPHR print from the same settings."""
import argparse
import base64
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile

import numpy as np

from kinematics import LIMITS, TOOL, forward, inverse

ROOT = Path(__file__).resolve().parent
PRODUCTION = ROOT.parent/'Globe_E27_Smoothed_Roll'
REFERENCE_BUILDER = PRODUCTION/'reference'/'build_trumpet.py'
SOURCE = ROOT/'reference'
STOP_LAYER = 30
BRIM_RINGS = 6
BRIM_WIDTH_MM = 23.4
LAST_BRIM_RING_OFFSET_MM = 3.9
WALL_START_ABOVE_BED = .15
START_FLOW_FRACTION = .10
LAYER_2_FLOW_FRACTION = .90
LIFT_MM = 50.
LIFT_SPEED = 50.
UPRIGHT_SPEED_DEG_S = 30.
WAYPOINT_STRIDE = 5
MAX_ORIENTATION_DEVIATION_DEG = 5.
SAMPLES_PER_ORIGINAL_MOVE = 4
STRAIGHT_SPEED = 6.
SCREW_COMMAND_PER_MM_S = 300

MOTION = re.compile(r'LIN (\{[^}]+\})(?: C_DIS)?')
SEND = re.compile(r'RET = EKI_SEND\("EthernetKRL_Config", "([^"]+)#"\)')
DISTANCE_TRIGGER = re.compile(r'TRIGGER WHEN DISTANCE=0 DELAY=0 DO (\w+)\(\) PRIO=-1')


def convert(reference, extra_waypoints, speeds, full):
    first = reference.index(';LAYER 1')
    taper = reference.index(';TOP TAPER')
    last = reference.index('WAIT SEC 0',taper) if full else reference.index(f';LAYER {STOP_LAYER+1}')
    purge_start = reference.index('RET = EKI_SEND("EthernetKRL_Config", "START_EXTRUDE#")')
    purge_move = purge_start-2
    assert MOTION.fullmatch(reference[purge_move]) and not reference[purge_move].endswith(' C_DIS')
    source, line_map = [], {}
    for i,line in enumerate(reference[:first],1):
        source.append(line)
        line_map[i] = len(source)
        if i==purge_move+1:
            # Halt at the exact purge target. Block EKI advance-run processing
            # until the operator has resumed from the halt.
            source.extend(['HALT','WAIT SEC 0'])
        if line == 'DECL EKI_STATUS RET':
            source += ['DECL SPL_VEL_MODE SPHR_MODE',
                       'DECL SPL_VEL_RESTR_STRUC SPHR_SAVED, SPHR_RESTR']
    source += ['SPHR_MODE = $SPL_VEL_MODE','SPHR_SAVED = $SPL_VEL_RESTR',
               'SPHR_RESTR = SPHR_SAVED','$SPL_VEL_MODE = #OPT',
               'SPHR_RESTR.ORI_VEL = #ON','SPHR_RESTR.ORI_ACC = #ON',
               'SPHR_RESTR.CART_ACC = #ON','$SPL_VEL_RESTR = SPHR_RESTR']
    spline = 'SPLINE WITH $ACC={CP 0.050, ORI1 150.0, ORI2 150.0}'
    source.append(spline)
    motion_lines = [i for i in range(first,last) if MOTION.fullmatch(reference[i])]
    ordinal = {line:i+1 for i,line in enumerate(motion_lines)}
    helpers = {}
    retained = []
    speed = STRAIGHT_SPEED
    def trigger(instruction):
        name = helpers.setdefault(instruction,f'SPHR_EVT_{len(helpers)+1}')
        source.append(f'  TRIGGER WHEN PATH=0 ONSTART DELAY=0 DO {name}() PRIO=-1')

    def set_flow(command):
        return f'RET = EKI_SEND("EthernetKRL_Config", "CHANGE_SPEED1 {command}#")'
    # Keep each layer/seam and process event at its original waypoint.
    for i in range(first,last):
        instruction = reference[i]
        if match := MOTION.fullmatch(instruction):
            index = ordinal[i]
            next_is_marker = i+1 == last or i+1 not in ordinal
            # The exact stop changes the fit at the endpoint. Retain the last
            # few targets so the final printed arc stays close to the design.
            test_end=STOP_LAYER*60
            dense = (index<=WAYPOINT_STRIDE or test_end-2*WAYPOINT_STRIDE<index<=test_end
                     or full and i>taper)
            if index % WAYPOINT_STRIDE == 0 or next_is_marker or dense or i+1 in extra_waypoints:
                source.append('  SPL '+match[1]+f' WITH $VEL.CP={speed/1000:.5f}')
                retained.append((i+1,len(source)))
        elif instruction.startswith(';'):
            source.append(instruction)
            if instruction.startswith(';LAYER '):
                layer=int(instruction.split()[-1])
                new_speed=speeds[layer]
                if layer>=10 and new_speed!=speed:
                    trigger(set_flow(round(SCREW_COMMAND_PER_MM_S*new_speed)))
                speed=new_speed
        else:
            if distance_trigger := DISTANCE_TRIGGER.fullmatch(instruction):
                sub = reference.index(f'DEF {distance_trigger[1]}()')
                instruction = reference[sub+2]
            command = SEND.fullmatch(instruction)
            if command is None:
                raise ValueError(f'Unexpected instruction at line {i+1}: {instruction}')
            if full and i>taper and command[1].startswith('CHANGE_SPEED1 '):
                screw=int(command[1].split()[-1])
                instruction=set_flow(round(screw*speed/STRAIGHT_SPEED))
            trigger(instruction)
    source.extend(['ENDSPLINE',';END PRINT','WAIT SEC 0',
                   'RET = EKI_SEND("EthernetKRL_Config", "STOP_EXTRUDE#")',set_flow(0)])
    source.extend(f'RET = EKI_SEND("EthernetKRL_Config", "SET_TEMP T{i} 0#")' for i in range(3))
    if full:
        source.extend([';WIPE',f'$VEL.CP = {speed/1000:.5f}',spline])
        source.extend('  SPL '+MOTION.fullmatch(reference[i])[1] for i in motion_lines if i>taper)
        source.append('ENDSPLINE')
    final_pose = MOTION.fullmatch(reference[motion_lines[-1]])[1]
    x,y,z,a,b,c = map(float,re.findall(r'[XYZABC] (-?[\d.]+)',final_pose))
    raised_xyz = f'X {x:.2f},Y {y:.2f},Z {z+LIFT_MM:.2f}'
    source.extend([';LIFT',f'$VEL.CP = {LIFT_SPEED/1000:g}',
                   f'LIN {{{raised_xyz},A {a:.2f},B {b:.2f},C {c:.2f}}}',
                   ';UPRIGHT',f'$VEL.ORI1 = {UPRIGHT_SPEED_DEG_S:.1f}',f'$VEL.ORI2 = {UPRIGHT_SPEED_DEG_S:.1f}',
                   f'LIN {{{raised_xyz},A -90.00,B 0.00,C 0.00}}',
                   'WAIT SEC 0','RET = EKI_SEND("EthernetKRL_Config", "SET_FAN1 0#")',
                   'RET = EKI_SEND("EthernetKRL_Config", "SET_FAN2 0#")',
                   '$SPL_VEL_RESTR = SPHR_SAVED','$SPL_VEL_MODE = SPHR_MODE','END'])
    for instruction,name in helpers.items():
        source.extend(['',f'DEF {name}()','DECL EKI_STATUS RET',instruction,'END'])
    return source, line_map, retained, np.array(motion_lines)+1, first+1


def cubic(knots, values, locations):
    """Natural cubic fit using a tridiagonal solve, shared by XYZ and orientation."""
    spacing = np.diff(knots)
    second = np.zeros_like(values)
    if len(knots) > 2:
        diagonal = 2*(spacing[:-1]+spacing[1:])
        rhs = 6*np.diff(np.diff(values,axis=0)/spacing[:,None],axis=0)
        for i in range(1,len(diagonal)):
            factor = spacing[i]/diagonal[i-1]
            diagonal[i] -= factor*spacing[i]
            rhs[i] -= factor*rhs[i-1]
        for i in range(len(diagonal)-1,-1,-1):
            second[i+1] = (rhs[i]-spacing[i+1]*second[i+2])/diagonal[i]
    segment = np.clip(np.searchsorted(knots,locations,side='right')-1,0,len(knots)-2)
    h = spacing[segment,None]
    b = (locations-knots[segment])[:,None]/h
    a = 1-b
    return (a*values[segment]+b*values[segment+1]
            +((a**3-a)*second[segment]+(b**3-b)*second[segment+1])*h*h/6)


def unit(q):
    return q/np.linalg.norm(q,axis=1,keepdims=True)


def tilt_from_vertical(q):
    return np.degrees(np.arccos(np.clip(1-2*np.sum(q[:,1:3]**2,axis=1),-1,1)))


def continuous_quaternions(q):
    q = unit(q)
    q[1:] *= np.cumprod(np.where(np.sum(q[1:]*q[:-1],axis=1)<0,-1.,1.))[:,None]
    return q


def rotation_matrices(q):
    w,x,y,z = q.T
    return np.column_stack([
        1-2*(y*y+z*z),2*(x*y-w*z),2*(x*z+w*y),
        2*(x*y+w*z),1-2*(x*x+z*z),2*(y*z-w*x),
        2*(x*z-w*y),2*(y*z+w*x),1-2*(x*x+y*y),
    ]).reshape(-1,3,3)


def replay(old, line_map, retained, original, first, speeds, source):
    source_refs = old[:,10].astype(int)
    motion_end = {line:index for index,line in enumerate(source_refs)}
    prefix = old[source_refs < first].copy()
    original_rows = np.vstack([prefix[-1],old[[motion_end[i] for i in original]]])
    path_param = np.r_[0.,np.cumsum(np.linalg.norm(np.diff(original_rows[:,:3],axis=0),axis=1))]
    kept = np.r_[0, np.searchsorted(original,[old_line for old_line,_ in retained])+1]
    assert kept[-1] == len(original)
    expanded = [np.linspace(path_param[a],path_param[b],(b-a)*SAMPLES_PER_ORIGINAL_MOVE+1)[1:]
                for a,b in zip(kept[:-1],kept[1:])]
    sample_param = np.r_[0.,np.concatenate(expanded)]
    source_q = continuous_quaternions(original_rows[:,-4:])
    xyz = cubic(path_param[kept], original_rows[kept,:3], sample_param)
    q = unit(cubic(path_param[kept],source_q[kept],sample_param))
    boundary_indices = np.r_[0,np.cumsum([len(values) for values in expanded])]
    assert np.allclose(xyz[boundary_indices],original_rows[kept,:3])
    assert np.allclose(q[boundary_indices],source_q[kept])

    # Evaluate the offline fit at the original dense targets.
    comparison_xyz = cubic(path_param[kept],original_rows[kept,:3],path_param)
    comparison_q = unit(cubic(path_param[kept],source_q[kept],path_param))
    positions = np.linalg.norm(comparison_xyz-original_rows[:,:3],axis=1)
    angles = np.degrees(2*np.arccos(np.clip(abs(np.sum(comparison_q*source_q,axis=1)),0,1)))
    span_speeds=[speeds[int(old[motion_end[line],9])] for line,_ in retained]
    sample_speeds=np.repeat(span_speeds,np.diff(boundary_indices))
    timing = prefix[-1,3] + np.r_[0.,np.cumsum(np.linalg.norm(np.diff(xyz,axis=0),axis=1)/sample_speeds)]
    path = []
    ori = []
    for (old_line,new_line),a,b in zip(retained,boundary_indices[:-1],boundary_indices[1:]):
        # A separate row at the segment start displays the new process command
        # at the same pose as its trigger.
        run = np.tile(old[motion_end[old_line],:15],(b-a+1,1))
        run[:,:3] = xyz[a:b+1]
        run[:,3] = timing[a:b+1]
        run[:,10] = new_line
        run[:,4] = speeds[int(run[0,9])]
        run[:,5] = np.rint(run[:,5]*run[:,4]/STRAIGHT_SPEED)
        path.append(run)
        ori.append(q[a:b+1])
    prefix[:,10] = [line_map[int(i)] for i in prefix[:,10]]
    wipe = None
    if ';WIPE' in source:
        taper=reference_taper_rows(original,source,retained)
        wipe=old[[motion_end[int(line)] for line in taper]]
    finish, finish_q = replay_finish(path[-1][-1],q[-1],source,wipe)
    data = np.vstack([prefix[:,:15],*path,finish])
    orientations = continuous_quaternions(np.vstack([prefix[:,-4:],*ori,finish_q]))
    assert np.min(np.diff(data[:,3]))>-1e-7
    data[:,3]=np.maximum.accumulate(data[:,3])
    fit = dict(
        maximumDroppedWaypointErrorMm=float(positions.max()),
        maximumDroppedOrientationErrorDeg=float(angles.max()),
        maximumDroppedTiltErrorDeg=float(np.abs(tilt_from_vertical(comparison_q)-tilt_from_vertical(source_q)).max()),
        shortestSplineChordMm=float(np.min(np.linalg.norm(np.diff(original_rows[kept,:3],axis=0),axis=1))),
    )
    deviating_lines = original[angles[1:]>MAX_ORIENTATION_DEVIATION_DEG]
    return data,orientations,fit,set(deviating_lines)


def reference_taper_rows(original,source,retained):
    first=source.index(';TOP TAPER')+1
    taper=np.array([old_line for old_line,new_line in retained if new_line>first])
    assert len(taper)==60 and np.array_equal(original[-len(taper):],taper)
    return taper


def replay_finish(last,q,source,wipe):
    pose=last.copy()
    pose[5]=pose[7]=0
    pose[8]=4
    pose[11:14]=0
    stop_line=source.index(';END PRINT')+3
    pose[4],pose[10]=0,stop_line
    history=[pose.copy()]
    orientations=[q]
    if wipe is not None:
        wipe_speed=last[4]
        first=source.index(';WIPE')
        lines=[i+1 for i in range(first,source.index(';LIFT',first))
               if source[i].startswith('  SPL ')]
        assert len(lines)==len(wipe)
        nodes=np.vstack([last[:3],wipe[:,:3]])
        knots=np.r_[0.,np.cumsum(np.linalg.norm(np.diff(nodes,axis=0),axis=1))]
        loc=np.linspace(knots[:-1],knots[1:],SAMPLES_PER_ORIGINAL_MOVE+1,axis=1)[:,1:].ravel()
        xyz=cubic(knots,nodes,loc)
        q_nodes=continuous_quaternions(np.vstack([q,wipe[:,-4:]]))
        quats=unit(cubic(knots,q_nodes,loc))
        times=last[3]+np.cumsum(np.linalg.norm(np.diff(np.vstack([nodes[0],xyz]),axis=0),axis=1)/wipe_speed)
        for point,attitude,t,line in zip(xyz,quats,times,np.repeat(lines,SAMPLES_PER_ORIGINAL_MOVE),strict=True):
            pose[:3],pose[3],pose[4],pose[10]=point,t,wipe_speed,line
            history.append(pose.copy())
            orientations.append(attitude)
    lift_line=source.index(';LIFT')+3
    pose[4],pose[10]=LIFT_SPEED,lift_line
    history.append(pose.copy())
    orientations.append(q)
    departure_z,departure_time=pose[2],pose[3]
    for offset in np.linspace(0.,LIFT_MM,math.ceil(LIFT_MM/2.)+1)[1:]:
        pose[2]=departure_z+offset
        pose[3]=departure_time+offset/LIFT_SPEED
        history.append(pose.copy())
        orientations.append(q)
    q_up=np.array([math.sqrt(.5),0.,0.,-math.sqrt(.5)])
    if np.dot(q_up,q)<0:
        q_up=-q_up
    angle=math.degrees(2*math.acos(np.clip(np.dot(q_up,q),0.,1.)))
    upright_line=source.index(';UPRIGHT')+4
    pose[4],pose[10]=0,upright_line
    history.append(pose.copy())
    orientations.append(q)
    rotation_start=pose[3]
    fractions=np.linspace(0.,1.,max(1,math.ceil(angle))+1)[1:]
    # Normalized interpolation samples the shortest rotation arc.
    upright_q=unit((1-fractions[:,None])*q+fractions[:,None]*q_up)
    for fraction,orientation in zip(fractions,upright_q):
        pose[3]=rotation_start+fraction*(angle/UPRIGHT_SPEED_DEG_S+.2)
        history.append(pose.copy())
        orientations.append(orientation)
    pose[6]=pose[14]=0
    pose[10]=upright_line+3
    history.append(pose)
    orientations.append(q_up)
    return np.array(history),np.array(orientations)


def previously_retained_waypoints(reference, full):
    # Reuse the original spline-node ordinals. The coordinates in the
    # revised profile can differ even when the programmed azimuth is the same.
    def wall_moves(lines, instruction):
        start=lines.index(';LAYER 1')
        end=lines.index(';TOP TAPER',start) if full else lines.index(f';LAYER {STOP_LAYER+1}',start)
        return [(i+1,lines[i].split(',A ',1)[0].split('{',1)[1])
                for i in range(start,end)
                if lines[i].lstrip().startswith(instruction+' ')]

    original_dense=wall_moves((REFERENCE_BUILDER.parent/'SPHR.SRC').read_text().splitlines(),'LIN')
    revised_dense=wall_moves(reference,'LIN')
    sparse=wall_moves((PRODUCTION/'SPHR.SRC').read_text().splitlines(),'SPL')
    retained=set()
    next_index=0
    for move,(_,old_target) in enumerate(original_dense):
        if next_index<len(sparse) and old_target==sparse[next_index][1]:
            if move<len(revised_dense):
                retained.add(revised_dense[move][0])
            next_index+=1
    assert next_index==len(sparse)
    return retained


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--full',action='store_true')
    args=parser.parse_args()
    subprocess.run([sys.executable,str(REFERENCE_BUILDER),'--output',str(SOURCE),
                    '--start-flow',str(START_FLOW_FRACTION),
                    '--layer-2-flow',str(LAYER_2_FLOW_FRACTION),
                    '--brim-rings',str(BRIM_RINGS),
                    '--brim-width',str(BRIM_WIDTH_MM),
                    '--last-brim-ring-offset',str(LAST_BRIM_RING_OFFSET_MM),
                    '--wall-start-height',str(WALL_START_ABOVE_BED),
                    '--wall-speed',str(STRAIGHT_SPEED),
                    '--profile',str(ROOT/'trumpet_profile.py')],check=True)
    output=ROOT.parent/'Globe_E27_Full' if args.full else ROOT
    output.mkdir(exist_ok=True)
    qa=json.loads((SOURCE/'QA.json').read_text())
    if not args.full:
        qa['layers']=STOP_LAYER
    plan=json.loads((PRODUCTION/'layer_speed_plan.json').read_text())
    previous_speeds=np.array([row['commandedMmS'] for row in plan['layers'][:qa['layers']]])
    low,high=previous_speeds[:STOP_LAYER].min(),previous_speeds[:STOP_LAYER].max()
    # Reuse the tested 6-to-21 speed scaling through layer 30, and carry
    # the original faster plan above 21 mm/s into the wider part.
    adapted=np.where(previous_speeds>high,previous_speeds,
                     np.interp(previous_speeds,[low,high],[STRAIGHT_SPEED,high]))
    speeds=np.r_[STRAIGHT_SPEED,np.round(adapted,1)]
    reference=(SOURCE/'SPHR.SRC').read_text().splitlines()
    reference_html=(SOURCE/'Non_Planar_Demo.html').read_text(encoding='utf8')
    payload=re.search(r'(?<![A-Za-z0-9+/])[A-Za-z0-9+/]{100000,}={0,2}',reference_html)[0]
    old=np.frombuffer(gzip.decompress(base64.b64decode(payload)),'<f4').reshape(-1,25).astype(float)
    retained_waypoints = previously_retained_waypoints(reference,args.full)
    extra_waypoints = retained_waypoints.copy()
    while True:
        source,line_map,retained,original,first = convert(reference,extra_waypoints,speeds,args.full)
        data,q,fit,deviating_lines = replay(old,line_map,retained,original,first,speeds,source)
        if not deviating_lines:
            break
        assert deviating_lines.isdisjoint(extra_waypoints)
        extra_waypoints.update(deviating_lines)
    fit['extraOrientationWaypoints'] = len(extra_waypoints-retained_waypoints)
    joints = inverse(data[:,:3],rotation_matrices(q))
    limits = LIMITS[:5]
    margins = np.minimum(joints[:,:5]-limits[:,0],limits[:,1]-joints[:,:5])
    fk = forward(joints)@TOOL
    fk_position_error = float(np.linalg.norm(fk[:,:3,3]-data[:,:3],axis=1).max())
    fk_orientation_error = float(np.abs(fk[:,:3,:3]-rotation_matrices(q)).max())
    assert fk_position_error<1e-6 and fk_orientation_error<1e-7
    durations = np.diff(data[:,3])
    moving = durations>0
    joint_steps = np.diff(joints,axis=0)
    peak_axis_speeds = np.max(np.abs(joint_steps[moving])/durations[moving,None],axis=0)

    ranges = []
    for layer in range(qa['layers']+1):
        deposited=data[(data[:,9]==layer)&(data[:,8]==1),3]
        ranges.append([float(deposited.min()),float(deposited.max())] if len(deposited) else None)
    wall, finish = (source.index(tag)+1 for tag in (';LAYER 1',';END PRINT'))
    wall_rows=(data[:,10]>wall)&(data[:,10]<finish)
    assert int(data[wall_rows,9].max())==qa['layers']
    assert np.all(data[wall_rows,4] == speeds[data[wall_rows,9].astype(int)])
    for layer,command in qa['layerStartScrewCommands'].items():
        assert set(data[wall_rows & (data[:,9]==int(layer)),5]) == {command}
    assert np.all(data[-1,11:14]==0) and data[-1,6]==0 and data[-1,14]==0
    assert np.all(data[-2,[6,14]]==[255,5])
    assert tilt_from_vertical(q[-1:])[0]<.001
    print_endpoint = data[np.flatnonzero(wall_rows)[-1],:3]
    if not args.full:
        qa['heightMm']=round(float(print_endpoint[2]-qa['printCentreWorldMm'][2]-qa['wallZOffsetMm']),2)

    src=('\r\n'.join(source)+'\r\n').encode('ascii')
    (output/'SPHR.SRC').write_bytes(src)
    shutil.copyfile(SOURCE/'SPHR.DAT',output/'SPHR.DAT')
    layer = 0
    gcode = []
    for line in (SOURCE/'Non_Planar_Demo.gcode').read_text().splitlines():
        if line.startswith(';LAYER '):
            layer=int(line.split()[-1])
            if layer>qa['layers']:
                break
        elif layer and line.startswith('G1 '):
            line=re.sub(r' F[\d.]+$',f' F{60*speeds[layer]:.0f}',line)
        gcode.append(line)
    (output/'Non_Planar_Demo.gcode').write_text('\n'.join(gcode)+'\n',encoding='ascii')
    meta=dict(qa,
        pointCount=len(data), modelMotions=qa['brimMotions']+len(retained),
        spiralMotions=len(retained)-(qa['taperMotions'] if args.full else 0),
        taperMotions=qa['taperMotions'] if args.full else 0, finishWipe=args.full,
        nominalSeconds=float(np.float32(data[-1,3])), layerRanges=ranges,
        motionModel='Offline natural cubic XYZ and quaternion fit',
        rollStrategy='Fixed world -Y reference projected into the nozzle plane',
        splineTargets=len(retained), originalWallTargets=len(original),
        printSpeedRangeMmS=[float(speeds[1:].min()),float(speeds[1:].max())],
        layerCommandedSpeedMmS=speeds[1:].tolist(), printScrewCommandRange=[round(SCREW_COMMAND_PER_MM_S*speeds[1:].min()),
                                                                round(SCREW_COMMAND_PER_MM_S*speeds.max())],
        finishLiftWorldZMm=LIFT_MM, printEndpointWorldMm=print_endpoint.tolist(), finishFansOff=True,
        splineFit=fit, jointLimitViolationVertices=int(np.any(margins<0,axis=1).sum()),
        jointLimitViolationSamples=int(np.any(margins<0,axis=1).sum()),
        minimumJointMarginsDeg=margins.min(axis=0).tolist(),
        maxVertexPositionErrorMm=fk_position_error, orientationMatrixError=fk_orientation_error,
        minimumWristBendDeg=float(np.abs(joints[:,4]).min()),
        peakSampledAxisSpeedDegS=float(peak_axis_speeds.max()),
        peakSampledSpeedPerAxisDegS=peak_axis_speeds.tolist(),
        largestVertexJointStepDeg=float(np.abs(joint_steps).max()),
        a6RangeDeg=[float(joints[:,5].min()),float(joints[:,5].max())],
        sourceHashes={name:hashlib.sha256((output/name).read_bytes()).hexdigest()
                      for name in ('SPHR.SRC','SPHR.DAT','Non_Planar_Demo.gcode')})
    meta.pop('maxMidpointPositionErrorMm',None)
    if not args.full:
        for key in ('taperSectors','clearOpeningMm','wallAngleFromVerticalDeg',
                    'nominalOutsideLipDiameterMm','lastTurnRiseMm','lastTurnRadialStepMm','lipDiameterMm'):
            meta.pop(key,None)
    cad=json.loads((ROOT/'cad_proxy.json').read_text())
    (output/'QA.json').write_text(json.dumps(meta,indent=2))
    packed=base64.b64encode(gzip.compress(np.column_stack([data,joints,q]).astype('<f4').tobytes())).decode()
    template=(REFERENCE_BUILDER.parent/'viewer_template.html').read_text(encoding='utf8')
    template=template.replace('Tilt held steady per turn, up to 45 deg',
                              ('Full-height print' if args.full else 'Stop after layer 30')+' | Offline spline estimate')
    template=template.replace('Nominal KRL replay from the first known Cartesian pose.',
                              'Offline spline estimate. KSS calculates its own trajectory.')
    template=template.replace('The nozzle points toward the previous spiral turn with a 45\u00b0 tilt cap and local tilt relief.',
                              'The nozzle tilt follows the per-turn SPHR plan.')
    html=template.replace('__LAYER_COUNT__',str(qa['layers'])).replace('__METADATA__',json.dumps(dict(meta,cad=cad),separators=(',',':'))).replace('__PACKED_DATA__',packed)
    (output/'Non_Planar_Demo.html').write_text(html,encoding='utf8')
    with zipfile.ZipFile(output/'SPHR.zip','w',compression=zipfile.ZIP_DEFLATED) as archive:
        for name in ('SPHR.SRC','SPHR.DAT'):
            archive.write(output/name,name)
    print(f'Spline targets {len(retained)}/{len(original)}; joint margins {margins.min(axis=0).round(3)}')
    print(fit)


if __name__=='__main__':
    main()
