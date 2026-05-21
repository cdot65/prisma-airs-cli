#!/usr/bin/env python3
"""Generate image-modality DLP test files, each hiding the same synthetic payload
in a different layer:

  1. dlp_img_1_metadata.jpg  - EXIF + XMP fields (plaintext, not rendered)
  2. dlp_img_2_container.jpg - JPEG COM segment + bytes appended after FFD9 EOI
  3. dlp_img_3_ocr.jpg       - synthetic text painted onto pixels (OCR required)
  4. dlp_img_4_stego.png     - LSB steganography (PNG: LSB can't survive JPEG)

All values are synthetic / reserved-for-testing.
"""
import struct
import numpy as np
from PIL import Image, ImageDraw, ImageFont
import piexif
import piexif.helper

BASE = "dlp_img_base.jpg"
W, H = 1200, 900

PAYLOAD_LINES = [
    "SSN: 078-05-1120",
    "Visa 4111 1111 1111 1111",
    "AWS_KEY AKIAIOSFODNN7EXAMPLE",
    "AWS_SECRET wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    "Email john.public@example.com  Phone (555) 010-0142",
    "Passport X12345678  DOB 1985-07-14",
]
BLOB = "\n".join(PAYLOAD_LINES)


# ---------------------------------------------------------------- base image
def make_base():
    # gradient + a few blocks so there is real pixel variance for steg/ocr bg
    x = np.linspace(0, 1, W)
    y = np.linspace(0, 1, H)
    xx, yy = np.meshgrid(x, y)
    r = (np.sin(xx * 6) * 0.5 + 0.5)
    g = (np.cos(yy * 5) * 0.5 + 0.5)
    b = (xx * yy)
    arr = np.dstack([r, g, b]) * 255
    arr = arr.astype(np.uint8)
    img = Image.fromarray(arr, "RGB")
    d = ImageDraw.Draw(img)
    for i, col in enumerate([(40, 60, 90), (150, 90, 60), (60, 120, 80)]):
        d.rectangle([80 + i * 360, 600, 380 + i * 360, 840], fill=col)
    img.save(BASE, quality=92)
    return img


def load_font(size):
    for p in [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/SFNS.ttf",
    ]:
        try:
            return ImageFont.truetype(p, size)
        except Exception:
            continue
    return ImageFont.load_default()


# ---------------------------------------------------------- 1. metadata
def build_metadata(base_img):
    out = "dlp_img_1_metadata.jpg"
    zeroth = {
        piexif.ImageIFD.ImageDescription: BLOB.encode("ascii", "replace"),
        piexif.ImageIFD.Artist: b"John Q. Public (synthetic)",
        piexif.ImageIFD.Copyright: b"SSN 078-05-1120; Visa 4111 1111 1111 1111",
        piexif.ImageIFD.XPComment: ("AWS_KEY AKIAIOSFODNN7EXAMPLE "
                                    "secret wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
                                    ).encode("utf-16le"),
        piexif.ImageIFD.XPKeywords: "PII;SSN;PAN;credentials".encode("utf-16le"),
    }
    exif_ifd = {
        piexif.ExifIFD.UserComment: piexif.helper.UserComment.dump(BLOB),
    }
    exif_bytes = piexif.dump({"0th": zeroth, "Exif": exif_ifd,
                              "GPS": {}, "1st": {}, "thumbnail": None})
    base_img.save(out, exif=exif_bytes, quality=92)

    # inject an XMP APP1 segment right after SOI
    xmp = (
        '<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?>'
        '<x:xmpmeta xmlns:x="adobe:ns:meta/">'
        '<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">'
        '<rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">'
        f'<dc:description><rdf:Alt><rdf:li xml:lang="x-default">{BLOB}'
        '</rdf:li></rdf:Alt></dc:description>'
        '<dc:subject><rdf:Bag>'
        '<rdf:li>Email john.public@example.com</rdf:li>'
        '<rdf:li>Passport X12345678 DOB 1985-07-14</rdf:li>'
        '</rdf:Bag></dc:subject>'
        '</rdf:Description></rdf:RDF></x:xmpmeta>'
        '<?xpacket end="w"?>'
    ).encode("utf-8")
    header = b"http://ns.adobe.com/xap/1.0/\x00"
    seg = header + xmp
    app1 = b"\xff\xe1" + struct.pack(">H", len(seg) + 2) + seg
    with open(out, "rb") as f:
        data = f.read()
    data = data[:2] + app1 + data[2:]   # after SOI (FFD8)
    with open(out, "wb") as f:
        f.write(data)
    return out


# ---------------------------------------------------------- 2. container
def build_container(base_img):
    out = "dlp_img_2_container.jpg"
    base_img.save(out, quality=92)
    with open(out, "rb") as f:
        data = f.read()
    # COM comment segment (0xFFFE) right after SOI
    com_text = ("DLP-COMMENT " + BLOB).encode("utf-8")
    com = b"\xff\xfe" + struct.pack(">H", len(com_text) + 2) + com_text
    data = data[:2] + com + data[2:]
    # plaintext appended after the final EOI marker (FFD9)
    trailer = ("\n--- appended after EOI ---\n" + BLOB + "\n").encode("utf-8")
    data = data + trailer
    with open(out, "wb") as f:
        f.write(data)
    return out


# ---------------------------------------------------------- 3. rendered/OCR
def build_ocr(base_img):
    out = "dlp_img_3_ocr.jpg"
    img = base_img.copy()
    d = ImageDraw.Draw(img)
    font = load_font(30)
    pad, lh = 24, 44
    box_h = pad * 2 + lh * len(PAYLOAD_LINES)
    d.rectangle([40, 40, W - 40, 40 + box_h], fill=(245, 245, 245))
    y = 40 + pad
    for line in PAYLOAD_LINES:
        d.text((60, y), line, fill=(10, 10, 10), font=font)
        y += lh
    img.save(out, quality=92)
    return out


# ---------------------------------------------------------- 4. LSB stego
def build_stego(base_img):
    out = "dlp_img_4_stego.png"   # lossless: LSB cannot survive JPEG
    arr = np.array(base_img.convert("RGB"), dtype=np.uint8)
    flat = arr.reshape(-1)
    payload = BLOB.encode("utf-8")
    header = struct.pack(">I", len(payload))      # 4-byte length prefix
    bits = np.unpackbits(np.frombuffer(header + payload, dtype=np.uint8))
    if bits.size > flat.size:
        raise SystemExit("payload too large for cover image")
    flat[:bits.size] = (flat[:bits.size] & 0xFE) | bits
    Image.fromarray(flat.reshape(arr.shape), "RGB").save(out)
    return out


def main():
    base = make_base()
    outs = [
        build_metadata(base),
        build_container(base),
        build_ocr(base),
        build_stego(base),
    ]
    print("Generated:", BASE)
    for o in outs:
        print("Generated:", o)


if __name__ == "__main__":
    main()
