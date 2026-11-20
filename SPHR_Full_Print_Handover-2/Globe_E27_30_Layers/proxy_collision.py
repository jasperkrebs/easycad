"""Convex proxy collision helpers shared by the robot screens."""
import math
import numpy as np

def unit_directions(vectors):
    lengths = np.linalg.norm(vectors, axis=-1)
    v = vectors[lengths > 1e-8] / lengths[lengths > 1e-8, None]
    k = np.argmax(np.abs(v), axis=1)
    v *= np.sign(v[np.arange(len(v)), k])[:, None]
    return np.unique(v.round(6), axis=0)

def body(part, frames, rotation, xyz):
    v = np.array(part['vertices'], dtype=float)
    f = np.array(part['faces'])
    triangles = v[f]
    normals = unit_directions(np.cross(triangles[:,1]-triangles[:,0], triangles[:,2]-triangles[:,0]))
    edges = unit_directions((triangles - np.roll(triangles, 1, axis=1)).reshape(-1,3))
    if 'frame' in part:
        matrix = frames[part['frame']]
        r, t = matrix[:,:3,:3], matrix[:,:3,3]
    else:
        r, t = rotation, xyz
    points = v @ r.transpose(0,2,1) + t[:,None,:]
    return dict(part=part, v=v, f=f, r=r, t=t, normals=normals, edges=edges,
                lo=points.min(1), hi=points.max(1))

def projection_overlap(a, b, axes):
    pa = a @ axes.transpose(0,2,1)
    pb = b @ axes.transpose(0,2,1)
    return np.all((pa.max(1) >= pb.min(1)-1e-5) & (pb.max(1) >= pa.min(1)-1e-5), axis=1)

def intersect(a, b):
    candidates, = np.where(np.all((a['hi'] >= b['lo']) & (b['hi'] >= a['lo']), axis=1))
    result = []
    for ids in np.array_split(candidates, max(1, math.ceil(len(candidates)/48))):
        if not len(ids):
            continue
        ra, rb = (p['r'][ids].transpose(0,2,1) for p in (a,b))
        va, vb = (p['v'] @ r + p['t'][ids,None,:] for p,r in ((a,ra),(b,rb)))
        normals = np.concatenate((a['normals'] @ ra, b['normals'] @ rb),axis=1)
        keep = projection_overlap(va,vb,normals)
        if not np.any(keep):
            continue
        ea, eb = a['edges'] @ ra[keep], b['edges'] @ rb[keep]
        cross = np.cross(ea[:,:,None,:], eb[:,None,:,:]).reshape(len(ea),-1,3)
        result.extend(ids[keep][projection_overlap(va[keep],vb[keep],cross)].tolist())
    return np.array(result, dtype=int)

