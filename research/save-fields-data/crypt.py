import struct, zlib, sys, os
from Crypto.Cipher import AES
BASE_KEY = bytes([0x35,0xEC,0x33,0x77,0xF3,0x5D,0xB0,0xEA,0xBE,0x6B,0x83,0x11,0x54,0x03,0xEB,0xFB,0x27,0x25,0x64,0x2E,0xD5,0x49,0x06,0x29,0x05,0x78,0xBD,0x60,0xBA,0x4A,0xA7,0x87])
SID = 76561198112570585
def key(sid=SID):
    k = bytearray(BASE_KEY); sb = struct.pack('<Q', sid)
    for i in range(8): k[i]^=sb[i]
    return bytes(k)
def decrypt_raw(data, sid=SID):
    return AES.new(key(sid), AES.MODE_ECB).decrypt(data)
def analyze(path):
    data = open(path,'rb').read()
    dec = decrypt_raw(data)
    pad = dec[-1]
    padok = 1 <= pad <= 16 and dec[-pad:] == bytes([pad])*pad
    body = dec[:-pad] if padok else dec
    d = zlib.decompressobj()
    yaml = d.decompress(body)
    unused = d.unused_data
    info = dict(size=len(data), zhdr=body[:2].hex(), pad=pad if padok else None, ylen=len(yaml), trailer=unused.hex())
    if len(unused)==8:
        a, l = struct.unpack('<II', unused)
        info['trailer_adler_ok'] = (a == zlib.adler32(yaml)); info['trailer_len_ok'] = (l==len(yaml))
    return info, yaml, body
if __name__=='__main__':
    for p in sys.argv[1:]:
        info,_,_ = analyze(p); print(os.path.basename(p), info)
