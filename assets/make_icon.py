from PIL import Image
import struct
import sys
import os

SRC = "icon_512.png"
ICO_OUT = "icon.ico"
ICNS_OUT = "icon.icns"
PNG_OUT = "icon_256.png"

ICO_SIZES = [16, 32, 48, 128, 256]
ICNS_SIZES = [16, 32, 64, 128, 256, 512, 1024]

ICNS_TYPE_MAP = {
    16: b"icp4",
    32: b"icp5",
    64: b"icp6",
    128: b"ic07",
    256: b"ic08",
    512: b"ic09",
    1024: b"ic10",
}

def load_source():
    im = Image.open(SRC).convert("RGBA")
    if im.size != (512, 512):
        im = im.resize((512, 512), Image.LANCZOS)
    return im

def build_ico(im):
    im.save(ICO_OUT, format="ICO", sizes=[(s, s) for s in ICO_SIZES])

def build_icns(im):
    entries = []
    for size in ICNS_SIZES:
        resized = im.resize((size, size), Image.LANCZOS)
        from io import BytesIO
        buf = BytesIO()
        resized.save(buf, format="PNG")
        data = buf.getvalue()
        entries.append((ICNS_TYPE_MAP[size], data))

    body = b""
    for ostype, data in entries:
        body += ostype + struct.pack(">I", len(data) + 8) + data

    header = b"icns" + struct.pack(">I", len(body) + 8)
    with open(ICNS_OUT, "wb") as f:
        f.write(header + body)

def build_png(im):
    resized = im.resize((256, 256), Image.LANCZOS)
    resized.save(PNG_OUT, format="PNG")

if __name__ == "__main__":
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    im = load_source()
    build_ico(im)
    build_icns(im)
    build_png(im)
    print("wrote", ICO_OUT, ICNS_OUT, PNG_OUT)
