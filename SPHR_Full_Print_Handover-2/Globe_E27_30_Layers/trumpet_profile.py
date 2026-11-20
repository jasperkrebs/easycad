"""Approximate profile inferred from the scaled diagram."""
import math
import numpy as np
pitch=2.5
flare_start=20.
target_crest_id=41.5
bead_width=4.5
stem_radius=(target_crest_id+bead_width)/2
lip_radius=77.  # 150 mm clear opening plus half the nominal 4 mm bead.
sphere_radius=95.
height=192.
sphere_centre_z=height-math.sqrt(sphere_radius**2-lip_radius**2)
join_z=50.
join_r=math.sqrt(sphere_radius**2-(join_z-sphere_centre_z)**2)
a0=math.asin((join_z-sphere_centre_z)/sphere_radius)
a1=math.asin((height-sphere_centre_z)/sphere_radius)
fillet_radius_mm=None
p0=np.array([stem_radius,flare_start])
p1=p0+[0,17.]
p3=np.array([join_r,join_z])
p2=p3-14*np.array([-math.sin(a0),math.cos(a0)])
t=np.linspace(0,1,4001)[:,None]
neck=(1-t)**3*p0+3*(1-t)**2*t*p1+3*(1-t)*t*t*p2+t**3*p3
da=3*((1-t)**2*(p1-p0)+2*(1-t)*t*(p2-p1)+t*t*(p3-p2))
a=np.linspace(a0,a1,16001)
sphere=np.column_stack([sphere_radius*np.cos(a),sphere_centre_z+sphere_radius*np.sin(a)])
points=np.vstack([neck,sphere[1:]])
angles=np.r_[np.arctan2(da[:,0],da[:,1]),-a[1:]]
length=np.r_[0.,np.cumsum(np.linalg.norm(np.diff(points,axis=0),axis=1))]
profile_length_mm=float(flare_start+length[-1])
layer_count=math.ceil(profile_length_mm/pitch)
lip_angle_deg=float(np.degrees(angles[-1]))
assert np.all(np.diff(points[:,1])>0)
def profile_at_distance(distance):
 if distance<=flare_start:return stem_radius,float(distance),0.
 s=min(distance-flare_start,length[-1])
 return tuple(float(np.interp(s,length,v)) for v in (points[:,0],points[:,1],angles))
def profile_at_height(z):
 return np.interp(z,points[:,1],points[:,0]),np.interp(z,points[:,1],angles)
