"""Re-emit a BL4 save YAML in the game's exact style from a composed node tree."""
import yaml, sys, glob, os
from yaml.nodes import ScalarNode, MappingNode, SequenceNode
def scal(n):
    if n.style == "'": return "'" + n.value.replace("'", "''") + "'"
    if n.style == '"': return '"' + n.value + '"'
    return n.value
def tagstr(n):
    return ' ' + n.tag if n.tag.startswith('!') and not n.tag.startswith('!!') and not n.tag.startswith('tag:') else ''
def emit_map(n, ind, out, first_prefix=None):
    for i,(k,v) in enumerate(n.value):
        pre = first_prefix if (i==0 and first_prefix is not None) else ' '*ind
        key = scal(k)
        if isinstance(v, ScalarNode):
            out.append(f"{pre}{key}: {scal(v)}")
        elif isinstance(v, MappingNode):
            out.append(f"{pre}{key}: {tagstr(v)}".rstrip(' ')+' ' if not tagstr(v) else f"{pre}{key}:{tagstr(v)}")
            if v.value: emit_map(v, ind+2, out)
        else:
            out.append(f"{pre}{key}:{tagstr(v)}" if tagstr(v) else f"{pre}{key}: ")
            emit_seq(v, ind, out)
def emit_seq(n, ind, out):
    for item in n.value:
        if isinstance(item, ScalarNode):
            out.append(' '*ind + '- ' + scal(item))
        elif isinstance(item, MappingNode):
            emit_map(item, ind+2, out, first_prefix=' '*ind+'- ')
        else:
            out.append(' '*ind + '- ')
            emit_seq(item, ind+2, out)
class L(yaml.SafeLoader): pass
def reemit(text):
    node = yaml.compose(text, Loader=L)
    out = []
    emit_map(node, 0, out)
    return '\n'.join(out)
if __name__ == '__main__':
    for p in sys.argv[1:]:
        t = open(p, encoding='utf-8', newline='').read()
        r = reemit(t)
        if r == t: print(os.path.basename(p), 'BYTE-IDENTICAL')
        else:
            a, b = t.split('\n'), r.split('\n')
            for i,(x,y) in enumerate(zip(a,b)):
                if x!=y: print(os.path.basename(p), 'first diff line', i+1, repr(x), repr(y)); break
            else: print(os.path.basename(p), 'len differ', len(a), len(b))
