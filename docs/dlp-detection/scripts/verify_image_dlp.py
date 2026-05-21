#!/usr/bin/env python3
"""Verify each image-modality DLP test file actually carries the payload."""
import struct
import numpy as np
from PIL import Image
import piexif

MARKERS = ["078-05-1120", "4111 1111 1111 1111", "AKIAIOSFODNN7EXAMPLE",
           "john.public@example.com", "X12345678"]


def found(text):
    return [m for m in MARKERS if m in text]


def check_metadata():
    print("\n[1] metadata  dlp_img_1_metadata.jpg")
    ex = piexif.load("dlp_img_1_metadata.jpg")
    blobs = []
    for ifd in ("0th", "Exif"):
        for tag, val in ex[ifd].items():
            if isinstance(val, bytes):
                blobs.append(val.decode("utf-16le", "ignore"))
                blobs.append(val.decode("latin-1", "ignore"))
    with open("dlp_img_1_metadata.jpg", "rb") as f:
        raw = f.read()
    has_xmp = b"ns.adobe.com/xap" in raw and b"dc:description" in raw
    text = " ".join(blobs) + raw.decode("latin-1", "ignore")
    print("    EXIF/XMP markers found:", found(text))
    print("    XMP packet present     :", has_xmp)
    print("    image still decodes    :", _decodes("dlp_img_1_metadata.jpg"))


def check_container():
    print("\n[2] container dlp_img_2_container.jpg")
    with open("dlp_img_2_container.jpg", "rb") as f:
        raw = f.read()
    text = raw.decode("latin-1", "ignore")
    has_com = b"\xff\xfe" in raw and b"DLP-COMMENT" in raw
    after_eoi = raw.split(b"\xff\xd9")[-1]
    print("    raw-byte markers found :", found(text))
    print("    COM segment present    :", has_com)
    print("    bytes after EOI marker :", len(after_eoi), "bytes",
          "->", found(after_eoi.decode("latin-1", "ignore")))
    print("    image still decodes    :", _decodes("dlp_img_2_container.jpg"))


def check_ocr():
    print("\n[3] ocr       dlp_img_3_ocr.jpg")
    import subprocess
    out = subprocess.run(["tesseract", "dlp_img_3_ocr.jpg", "-", "--psm", "6"],
                         capture_output=True, text=True)
    text = out.stdout
    print("    OCR-recovered markers  :", found(text))


def check_stego():
    print("\n[4] stego     dlp_img_4_stego.png")
    arr = np.array(Image.open("dlp_img_4_stego.png").convert("RGB"), dtype=np.uint8)
    flat = arr.reshape(-1)
    bits = flat & 1
    by = np.packbits(bits[: (flat.size // 8) * 8]).tobytes()
    n = struct.unpack(">I", by[:4])[0]
    payload = by[4:4 + n].decode("utf-8", "ignore")
    print("    LSB-decoded length     :", n, "bytes")
    print("    LSB-decoded markers    :", found(payload))


def _decodes(path):
    try:
        Image.open(path).load()
        return True
    except Exception as e:
        return f"FAIL {e}"


if __name__ == "__main__":
    check_metadata()
    check_container()
    check_ocr()
    check_stego()
    print("\n(all 5 markers per file == full coverage)")
