import sys, re, collections, time, pickle
sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent))
from cw import P, get, get_all, items
def top_index(b):
    tok=re.compile(rb'\s+|#[^\n]*|"(?:\\.|[^"\\])*"|[{}]|[<>!?]?=|[^\s{}=<>!?"#]+')
    spans=[]; depth=0; pending=None; awaiting=False; cur=None
    for m in tok.finditer(b):
        t=m.group()
        if t[:1].isspace() or t[:1]==b'#': continue
        if depth>0:
            if t==b'{': depth+=1
            elif t==b'}':
                depth-=1
                if depth==0 and cur: spans.append((cur[0],cur[1],m.end(),True)); cur=None
            continue
        if t.endswith(b'=') and len(t)<=2 and pending is not None: awaiting=True; continue
        if t==b'{':
            depth=1
            if awaiting: cur=(pending,m.start())
            pending=None; awaiting=False; continue
        if awaiting: spans.append((pending,m.start(),m.end(),False)); pending=None; awaiting=False
        else: pending=t
    return spans
def load(path):
    data=open(path,'rb').read()
    assert data[:7]==b'HOI4txt'; data=data[7:]
    idx=top_index(data)
    return data, idx
def section(data, sp): return data[sp[1]+1:sp[2]-1] if sp[3] else data[sp[1]:sp[2]]
