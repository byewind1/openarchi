#!/usr/bin/env python3
"""Run from repository root. Compare measured AC29 polygon unions to the
unbounded angular-neighbour hypothesis; this is observation analysis, not a
substitute for the production geometry regression tests (bounded sectors).
"""
import math,json
C=json.load(open('docs/features/wall-junction-cleanup/archicad-cases.json'))['cases'];D=json.load(open('docs/features/wall-junction-cleanup/archicad-details.json'))['detailsOfElements']
def add(a,b):return(a[0]+b[0],a[1]+b[1])
def mul(a,b):return(a[0]*b,a[1]*b)
def cross(a,b):return a[0]*b[1]-a[1]*b[0]
def intersect(a,v,b,w):
 den=cross(v,w)
 if abs(den)<1e-8:return None
 t=cross((b[0]-a[0],b[1]-a[1]),w)/den
 return add(a,mul(v,t))
def contains(p,poly):
 inside=False
 for a,b in zip(poly,poly[1:]+poly[:1]):
  if (a[1]>p[1])!=(b[1]>p[1]) and p[0] < (b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0]:inside=not inside
 return inside
def sub(a,b):return(a[0]-b[0],a[1]-b[1])
def boundary(polys):
 es=[(a,b) for p in polys for a,b in zip(p,p[1:]+p[:1]) if math.dist(a,b)>1e-9];out=[]
 for a,b in es:
  v=sub(b,a);vv=sum(x*x for x in v);ts=[0,1]
  for c,d in es:
   w=sub(d,c);den=cross(v,w)
   if abs(den)>1e-9:
    t=cross(sub(c,a),w)/den;u=cross(sub(c,a),v)/den
    if -1e-7<=t<=1+1e-7 and -1e-7<=u<=1+1e-7:ts.append(max(0,min(1,t)))
   elif abs(cross(sub(c,a),v))<1e-8:
    for e in (c,d):
     t=sum(x*y for x,y in zip(sub(e,a),v))/vv
     if 0<t<1:ts.append(t)
  ts=sorted(set(round(t,10)for t in ts))
  for t,u in zip(ts,ts[1:]):
   if u-t<1e-8:continue
   mid=add(a,mul(v,(t+u)/2));n=mul((-v[1],v[0]),1e-7/math.sqrt(vv))
   left=any(contains(add(mid,n),p)for p in polys);right=any(contains(sub(mid,n),p)for p in polys)
   if left==right:continue
   e=(add(a,mul(v,t)),add(a,mul(v,u)))
   if right:e=e[::-1]
   if not any(math.dist(e[0],x[0])<1e-7 and math.dist(e[1],x[1])<1e-7 for x in out):out.append(e)
 return out
base=0
for case in C:
 o=case['origin'];ends=[(e[0]-o[0],e[1]-o[1]) for e in case['ends']]
 dirs=[mul(e,1/math.hypot(*e)) for e in ends];ns=[(-d[1],d[0])for d in dirs]
 order=sorted(range(len(dirs)),key=lambda i:math.atan2(dirs[i][1],dirs[i][0]));our=[]
 for k,i in enumerate(order):
  j=order[(k+1)%len(order)];l=order[(k-1)%len(order)]
  left=intersect(mul(ns[i],.15),dirs[i],mul(ns[j],-.15),dirs[j]) or mul(ns[i],.15)
  right=intersect(mul(ns[i],-.15),dirs[i],mul(ns[l],.15),dirs[l]) or mul(ns[i],-.15)
  our.append([left,add(ends[i],mul(ns[i],.15)),add(ends[i],mul(ns[i],-.15)),right,(0,0)])
 ac=[[(p['x']-o[0],p['y']-o[1])for p in poly['coordinates'][:-1]]for w in D[base:base+len(ends)]for poly in w['floorPlanPolygons']];base+=len(ends)
 print(case['name'])
 for name,p in [('AC',ac),('angular',our)]:
  edges=boundary(p);print(name,'area',round(sum(cross(a,b)for a,b in edges)/2,9),'local edges',[(tuple(round(x,6)for x in a),tuple(round(x,6)for x in b))for a,b in edges if math.hypot(*a)<1.5 and math.hypot(*b)<1.5])
