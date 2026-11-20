import math
import numpy as np
A=np.array([175,1290,50,0,0,0.]);DD=np.array([575,0,0,1035,0,185.]);AL=np.array([math.pi/2,0,math.pi/2,-math.pi/2,math.pi/2,0])
SEED=np.array([-10.7617,-14.3847,124.2567,-100.1345,-86.3609,160.4508])
LIMITS=np.array([[-185,185],[-175,60],[-120,170],[-180,180],[-125,125],[-450,450]])
def rot(abc):
 a,b,c=np.radians(abc);ca,sa,cb,sb,cc,sc=np.cos(a),np.sin(a),np.cos(b),np.sin(b),np.cos(c),np.sin(c)
 return np.array([[ca*cb,ca*sb*sc-sa*cc,ca*sb*cc+sa*sc],[sa*cb,sa*sb*sc+ca*cc,sa*sb*cc-ca*sc],[-sb,cb*sc,cb*cc]])
TOOL=np.eye(4);TOOL[:3,:3]=rot([-180,-90,0]);TOOL[:3,3]=[-508,3.5,139.34]
R=rot([-82.6592,-.0415,-.0234]);FR=R@TOOL[:3,:3].T
OFFSET=-FR@TOOL[:3,3]-185*FR[:,2]
def forward(j,n=6):
 j=np.atleast_2d(j);mat=np.tile(np.eye(4),(len(j),1,1))
 for i in range(n):
  q=-np.radians(j[:,i])+(math.pi/2 if i==2 else 0);c,s=np.cos(q),np.sin(q);ca,sa=math.cos(AL[i]),math.sin(AL[i])
  t=np.zeros_like(mat);t[:,0,0]=c;t[:,0,1]=-s*ca;t[:,0,2]=s*sa;t[:,0,3]=A[i]*c
  t[:,1,0]=s;t[:,1,1]=c*ca;t[:,1,2]=-c*sa;t[:,1,3]=A[i]*s
  t[:,2,1]=sa;t[:,2,2]=ca;t[:,2,3]=DD[i];t[:,3,3]=1
  mat=mat@t
 return mat
def inverse(xyz, rotations=None):
 fr=FR if rotations is None else rotations@TOOL[:3,:3].T
 offset=-fr@TOOL[:3,3]-185*fr[...,2]
 w=xyz+offset;r=np.hypot(w[:,0],w[:,1])-175;z=w[:,2]-575
 l1=1290.;l2=math.hypot(50,1035);delta=math.atan2(-1035,50)
 co=(r*r+z*z-l1*l1-l2*l2)/(2*l1*l2)
 assert np.all(np.abs(co)<=1+1e-9),'Position outside geometric reach'
 phi=-np.arccos(np.clip(co,-1,1));q2=phi-delta
 q1=np.arctan2(z,r)-np.arctan2(l2*np.sin(phi),l1+l2*np.cos(phi))
 j=np.zeros((len(xyz),6));j[:,0]=-np.degrees(np.arctan2(w[:,1],w[:,0]));j[:,1]=-np.degrees(q1);j[:,2]=90-np.degrees(q2)
 rr=forward(j,3)[:,:3,:3].transpose(0,2,1)@fr
 j[:,3]=-np.degrees(np.arctan2(rr[:,1,2],rr[:,0,2]));j[:,4]=-np.degrees(np.arccos(np.clip(rr[:,2,2],-1,1)))
 j[:,5]=-np.degrees(np.arctan2(rr[:,2,1],-rr[:,2,0]))
 j=np.degrees(np.unwrap(np.radians(j),axis=0))
 j+=np.round((SEED-j[0])/360)*360
 return j
