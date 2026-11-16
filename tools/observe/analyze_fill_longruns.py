"""F-001 run 6 analysis: the long-run rule of the anti-aliased smoothing of scaled fill areas.

Builds on analyze_fill_smoothing.py (outline tracing, run midpoints, exact-coverage raster).
Model (see spec F-001 run 6): every straight run of the outline contributes its midpoint;
at a corner between two runs of length >= 2 the corner is kept, except that a run whose other
end is a step continues the line from the step through its midpoint to the corner when that
moves the boundary into the shape (the fill grows); a segment between a step and a run that is
not so extended is translated 0.25 px along that run, away from the step (monotone
staircases are left alone).

usage: analyze_fill_longruns.py AA_PNG BINARY_PNG Y0 Y1 X0 X1
"""
import sys
import numpy as np
from PIL import Image
sys.path.insert(0, __import__("os").path.dirname(__file__))
from analyze_fill_smoothing import contours, raster
def runs_of(loop):
    n=len(loop); P=[np.array(p,float) for p in loop]
    d=[tuple((P[(i+1)%n]-P[i]).astype(int)) for i in range(n)]
    k=0
    while d[k]==d[k-1]: k+=1
    P=P[k:]+P[:k]; d=d[k:]+d[:k]
    R=[]; i=0
    while i<n:
        j=i
        while j+1<n and d[j+1]==d[i]: j+=1
        R.append((P[i], P[(j+1)%n], np.array(d[i],float), j-i+1)); i=j+1
    return R
def cross(a,b): return a[0]*b[1]-a[1]*b[0]
def intersect(p,u,q,v):
    den=cross(u,v)
    if abs(den)<1e-12: return None
    t=cross(q-p,v)/den; return p+t*u
def polygon(loop, L=2):
    R=runs_of(loop); m=len(R)
    M=[(a+b)/2 for a,b,_,_ in R]
    long_=[r[3]>=L for r in R]
    special=[long_[k] and long_[(k+1)%m] for k in range(m)]   # corner at end of run k
    out=[]
    for k in range(m):
        out.append(M[k])
        if not special[k]: continue
        a,b,dk,lk=R[k]; k1=(k+1)%m; a1,b1,dk1,lk1=R[k1]
        C=b.copy()
        # mask on the right of the walking direction (y down): right normal of d = (-dy, dx)
        def right(d): return np.array([-d[1],d[0]])
        # tilt of run k at its end: line from previous vertex through M_k, if the previous corner is not special
        Qk=None
        if not special[k-1]:
            u=M[k]-M[k-1]
            q=intersect(M[k],u,C,dk1)
            if q is not None and (q-C)@right(dk)>1e-9: Qk=q        # boundary moved into the mask: fill grows
        Qk1=None
        if not special[k1]:
            u=M[k1]-M[(k1+1)%m]
            q=intersect(M[k1],u,C,dk)
            if q is not None and (q-C)@right(dk1)>1e-9: Qk1=q
        if Qk is None and Qk1 is None: out.append(C)
        elif Qk1 is None: out.append(Qk)
        elif Qk is None: out.append(Qk1)
        else:
            out.append(intersect(M[k],M[k]-M[k-1],M[k1],M[k1]-M[(k1+1)%m]))
    return out
def model(mask, L=2):
    return raster([polygon(l,L) for l in contours(mask)], mask.shape[0], mask.shape[1])

def polygon2(loop, L=2, sh=0.25):
    """polygon() plus: a segment between a 1-run (step) midpoint and the midpoint of a long run B
    is translated by `sh` along B, away from the step, when B is not extended to a corner on
    that segment's line (B's other end is a flat corner, or another step in the opposite
    direction). Monotone staircases are left alone."""
    R=runs_of(loop); m=len(R)
    M=[(a+b)/2 for a,b,_,_ in R]
    lg=[r[3]>=L for r in R]
    special=[lg[k] and lg[(k+1)%m] for k in range(m)]
    base=polygon(loop,L)
    # rebuild with explicit segment translations
    def right(d): return np.array([-d[1],d[0]])
    pts=[]
    for k in range(m):
        k1=(k+1)%m
        pts.append(('M',k,M[k]))
        if special[k]:
            pts.append(('C',k,None))
    # compute the base corner points from polygon() in the same order
    bi=0; out=[]
    for kind,k,p in pts:
        out.append([kind,k,base[bi].copy()]); bi+=1
    # translations
    res=[]
    n=len(out)
    for idx in range(n):
        kind,k,p=out[idx]; res.append(p)
    # segment list between consecutive M vertices of a step (1-run) and a long run
    segs=[]
    for idx in range(n):
        a=out[idx]; b=out[(idx+1)%n]
        if a[0]=='M' and b[0]=='M':
            ka,kb=a[1],b[1]
            la,lb=R[ka][3],R[kb][3]
            if la<L and lb>=L: step,run,away=ka,kb,1
            elif lb<L and la>=L: step,run,away=kb,ka,-1
            else: continue
            # neighbours of the long run
            prv=R[(run-1)%m]; nxt=R[(run+1)%m]
            other = nxt if run==kb else prv          # the run on the far side of B
            otherk=(run+1)%m if run==kb else (run-1)%m
            corner_far = lg[otherk]
            stepdir=R[step][2]; otherdir=R[otherk][2]
            if corner_far:
                # extended to the far corner? (polygon() used the tilt there)
                c_idx = (idx+1)%n if run==kb else idx
                # find the corner entry adjacent on B's far side
                j=(idx+2)%n if run==kb else (idx-1)%n
                if out[j][0]=='C':
                    Cp=out[j][2]; Cg=R[run][1] if run==kb else R[run][0]
                    if np.abs(Cp-Cg).max()>1e-9: continue   # extended: no translation
                d=R[run][2]
            else:
                if np.array_equal(otherdir, stepdir): continue  # monotone staircase
                d=R[run][2]
            # translate along B, away from the step
            t = d*sh if run==kb else -d*sh
            segs.append((idx,(idx+1)%n,t))
    # apply: replace the segment endpoints by translated copies (inserting vertices)
    newpoly=[]
    seg_at={i:(j,t) for i,j,t in segs}
    for idx in range(n):
        p=out[idx][2]
        if idx in seg_at:
            j,t=seg_at[idx]
            newpoly.append(p); newpoly.append(p+t); newpoly.append(out[j][2]+t)
        else:
            newpoly.append(p)
    # merge overlapping translated segments meeting at the same run midpoint (V apex): intersect
    return newpoly
def model2(mask, L=2):
    return raster([polygon2(l,L) for l in contours(mask)], mask.shape[0], mask.shape[1])

if __name__ == "__main__":
    aa = np.array(Image.open(sys.argv[1]))[..., 3].astype(int)
    b = np.array(Image.open(sys.argv[2]))[..., 3] > 0
    y0, y1, x0, x1 = map(int, sys.argv[3:7])
    pad = 3
    mask = ~b[y0 - pad:y1 + pad, x0 - pad:x1 + pad]
    m = np.round((1 - model2(mask)[pad:-pad, pad:-pad]) * 255).astype(int)
    d = aa[y0:y1, x0:x1]
    soft = ((d > 0) & (d < 255)) | ((m > 0) & (m < 255))
    e = np.abs(m - d)[soft]
    print("soft", int(soft.sum()), "exact", int((e == 0).sum()), "off by 1", int((e == 1).sum()),
          "more", int((e > 1).sum()), "max", int(e.max()) if e.size else 0)
