"""Full offline pipeline: .sav -> AES/zlib -> YAML node tree -> game-style emit -> zlib(6)+len4 -> AES -> .sav ; compare bytes."""
import sys, os, glob, zlib, struct, re
from crypt import analyze, key
from emit import reemit
from Crypto.Cipher import AES
def encrypt(yaml_bytes):
    comp = zlib.compress(yaml_bytes, 6) + struct.pack('<I', len(yaml_bytes))
    pad = 16 - len(comp) % 16
    return AES.new(key(), AES.MODE_ECB).encrypt(comp + bytes([pad]) * pad)
ok = 0
for p in sorted(glob.glob(r'C:\Code\bl4-mods\bl4-save-editor\testdata\saves\*.sav')):
    info, y, _ = analyze(p)
    text = y.decode('utf-8')
    out = encrypt(reemit(text).encode('utf-8'))
    same = out == open(p, 'rb').read()
    ok += same
    print(f'{os.path.basename(p):26} {"BYTE-IDENTICAL" if same else "differs (tool-written source)"}')
print(ok, 'identical')
# surgical edit: cash in 11.sav
info, y, _ = analyze(r'C:\Code\bl4-mods\bl4-save-editor\testdata\saves\11.sav')
t = y.decode()
t2 = re.sub(r'(\n    cash: )\d+', r'\g<1>999999999', t, count=1)
open('rt/11.cash.sav', 'wb').write(encrypt(t2.encode()))
