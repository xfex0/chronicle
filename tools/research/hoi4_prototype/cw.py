# Minimal Python port of paradox-parser semantics (lexer + parser) to validate AST-shape assumptions.
import re
TOK=re.compile(rb'\s+|#[^\n]*|"(?:\\.|[^"\\])*"|\{|\}|<=|>=|!=|\?=|==|=|<|>|[^\s{}=<>"#]+')
TAGS={b'rgb',b'hsv',b'hsv360',b'hex',b'LIST'}
def tokens(b):
    for m in TOK.finditer(b):
        t=m.group()
        if t[:1].isspace() or t[:1]==b'#': continue
        yield t
class P:
    def __init__(s,b): s.it=tokens(b); s.peek=None
    def bump(s):
        if s.peek is not None: t,s.peek=s.peek,None; return t
        return next(s.it,None)
    def pk(s):
        if s.peek is None: s.peek=next(s.it,None)
        return s.peek
    def entries(s,nested):
        out=[]
        while True:
            t=s.bump()
            if t is None: return out
            if t==b'}':
                if nested: return out
                continue
            if t==b'{': out.append(('item',s.entries(True))); continue
            if t in (b'=',b'<',b'>',b'<=',b'>=',b'!=',b'?=',b'=='): continue
            key=t.strip(b'"') if t.startswith(b'"') else t
            n=s.pk()
            if n in (b'=',b'<',b'>',b'<=',b'>=',b'!=',b'?=',b'=='):
                s.bump(); out.append(('pair',key,s.value()))
            elif n==b'{' and t in TAGS: s.bump(); out.append(('item',('tag',t,s.entries(True))))
            else: out.append(('item',key))
    def value(s):
        t=s.bump()
        if t==b'{': return s.entries(True)
        v=t.strip(b'"') if t.startswith(b'"') else t
        if t in TAGS and s.pk()==b'{': s.bump(); return ('tag',t,s.entries(True))
        return v
def get(c,k): 
    for e in c:
        if e[0]=='pair' and e[1]==k: return e[2]
def get_all(c,k): return [e[2] for e in c if e[0]=='pair' and e[1]==k]
def items(c): return [e[1] for e in c if e[0]=='item']
