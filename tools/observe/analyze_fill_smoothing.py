"""F-001 run 5 analysis: the anti-aliased smoothing of scaled fill areas.

Model checked here (exact on regular and irregular staircases): trace the outline of the
binary area (pixel edges), take the midpoint of every maximal straight run, join the midpoints
into a polygon and rasterise it with exact area coverage (round(255·coverage)). It does NOT
hold where long runs meet (corners of long edges stay sharp in CSP; single steps, notches and
bumps become ramps whose ends are not yet explained): see the spec.

usage: analyze_fill_smoothing.py AA_PNG BINARY_PNG Y0 Y1 X0 X1
prints model vs CSP for the soft pixels in the window.
"""
import sys
import numpy as np
from PIL import Image
def contours(b):
    """Directed boundary edges (inside on the left in y-down coords → we keep a consistent orientation), linked into loops of vertices."""
    H,W=b.shape; p=np.pad(b,1)
    nxt={}
    # edges as (start vertex, end vertex); orientation: inside on the right when walking (y down)
    for y in range(H):
        for x in range(W):
            if not p[y+1,x+1]: continue
            if not p[y,x+1]:   nxt.setdefault((x,y),[]).append((x+1,y))       # top edge, walk +x
            if not p[y+1,x+2]: nxt.setdefault((x+1,y),[]).append((x+1,y+1))   # right edge, walk +y
            if not p[y+2,x+1]: nxt.setdefault((x+1,y+1),[]).append((x,y+1))   # bottom edge, walk -x
            if not p[y+1,x]:   nxt.setdefault((x,y+1),[]).append((x,y))       # left edge, walk -y
    loops=[]
    while nxt:
        s=next(iter(nxt)); loop=[s]; cur=s; prevd=None
        while True:
            outs=nxt[cur]
            if len(outs)==1: n=outs[0]
            else:
                # pinch vertex: turn right (keep diagonal pixels separate)
                d=prevd
                pref=[(-d[1],d[0]),d,(d[1],-d[0])]  # right turn first (y down: right of (dx,dy) is (-dy,dx))
                n=None
                for pd in pref:
                    c=(cur[0]+pd[0],cur[1]+pd[1])
                    if c in outs: n=c; break
            outs.remove(n)
            if not outs: del nxt[cur]
            prevd=(n[0]-cur[0],n[1]-cur[1]); cur=n
            if cur==s and (s not in nxt): break
            if cur==s: break
            loop.append(cur)
        loops.append(loop)
    return loops
def runs_mid(loop):
    n=len(loop); pts=[np.array(v,float) for v in loop]
    dirs=[tuple((pts[(i+1)%n]-pts[i]).astype(int)) for i in range(n)]
    # rotate so a run starts at index 0
    k=0
    while dirs[k]==dirs[k-1]:
        k+=1
        if k>=n: return [pts[0]]
    pts=pts[k:]+pts[:k]; dirs=dirs[k:]+dirs[:k]
    mids=[]; i=0
    while i<n:
        j=i
        while j+1<n and dirs[j+1]==dirs[i]: j+=1
        a=pts[i]; e=pts[(j+1)%n] if j+1<n else pts[0]
        mids.append((a+e)/2); i=j+1
    return mids
def raster(polys,H,W):
    WW=W+3
    a=np.zeros(H*WW)
    for poly in polys:
        m=len(poly)
        for i in range(m):
            draw_line(a,poly[i],poly[(i+1)%m],H,WW)
    acc=np.cumsum(a.reshape(H,WW),axis=1)[:,:W]
    return np.clip(np.abs(acc),0,1)
def draw_line(a,p0,p1,H,WW):
    x0_,y0_=float(p0[0]),float(p0[1]); x1_,y1_=float(p1[0]),float(p1[1])
    if y0_==y1_: return
    if y0_<y1_: d0=1.0
    else: d0=-1.0; x0_,y0_,x1_,y1_=x1_,y1_,x0_,y0_
    dxdy=(x1_-x0_)/(y1_-y0_)
    x=x0_
    ys=int(np.floor(y0_)) if y0_>0 else 0
    if y0_<0: x-=y0_*dxdy
    for y in range(ys, min(H,int(np.ceil(y1_)))):
        ls=y*WW
        dy=min(y+1.0,y1_)-max(float(y),y0_)
        xn=x+dxdy*dy
        d=dy*d0
        xa,xb=(x,xn) if x<xn else (xn,x)
        x0f_=np.floor(xa); x0i=int(x0f_)
        x1c=np.ceil(xb); x1i=int(x1c)
        if x1i<=x0i+1:
            xmf=0.5*(x+xn)-x0f_
            a[ls+x0i]+=d-d*xmf
            a[ls+x0i+1]+=d*xmf
        else:
            s=1.0/(xb-xa)
            x0f=xa-x0f_
            a0=0.5*s*(1-x0f)**2
            x1f=xb-x1c+1.0
            am=0.5*s*x1f**2
            a[ls+x0i]+=d*a0
            if x1i==x0i+2:
                a[ls+x0i+1]+=d*(1.0-a0-am)
            else:
                a1=s*(1.5-x0f)
                a[ls+x0i+1]+=d*(a1-a0)
                for xi in range(x0i+2,x1i-1): a[ls+xi]+=d*s
                a2=a1+(x1i-x0i-3)*s
                a[ls+x1i-1]+=d*(1.0-a2-am)
            a[ls+x1i]+=d*am
        x=xn

if __name__ == "__main__":
    aa = np.array(Image.open(sys.argv[1]))[..., 3].astype(int)
    b = np.array(Image.open(sys.argv[2]))[..., 3] > 0
    y0, y1, x0, x1 = map(int, sys.argv[3:7])
    pad = 3
    win = ~b[y0 - pad:y1 + pad, x0 - pad:x1 + pad]
    polys = [runs_mid(l) for l in contours(win)]
    cov = raster(polys, win.shape[0], win.shape[1])[pad:-pad, pad:-pad]
    model = np.round((1 - cov) * 255).astype(int)
    csp = aa[y0:y1, x0:x1]
    soft = (csp > 0) & (csp < 255)
    print("soft pixels", int(soft.sum()), "model differs on", int((model != csp)[soft].sum()))
