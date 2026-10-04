import yaml, glob, os, re
class TagList(list): pass
def tags_ctor(loader, node):
    return TagList(loader.construct_sequence(node))
yaml.SafeLoader.add_constructor('!tags', tags_ctor)
YDIR = r'C:\Code\bl4-mods\bl4-save-editor\testdata\yaml'
def load(name):
    return yaml.safe_load(open(os.path.join(YDIR, name), encoding='utf-8'))
def chars():
    out = {}
    for f in sorted(glob.glob(os.path.join(YDIR, '*.yaml'))):
        b = os.path.basename(f)
        if b == 'profile.yaml': continue
        out[b[:-5]] = load(b)
    return out
def paths(d, prefix='', depth=99, collapse=True):
    """yield (path, value) for leaves; collapse slot_N / numeric keys"""
    if depth == 0 or not isinstance(d, (dict, list)):
        yield prefix, d; return
    if isinstance(d, dict):
        if not d: yield prefix, {}
        for k, v in d.items():
            ks = str(k)
            if collapse: ks = re.sub(r'^slot_\d+$', 'slot_N', ks)
            yield from paths(v, f'{prefix}.{ks}' if prefix else ks, depth-1, collapse)
    else:
        if not d: yield prefix, []
        for i, v in enumerate(d):
            yield from paths(v, f'{prefix}[]' if collapse else f'{prefix}[{i}]', depth-1, collapse)
