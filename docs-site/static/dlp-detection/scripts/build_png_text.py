#!/usr/bin/env python3
"""PNG textual-metadata DLP test: same synthetic payload embedded in PNG
tEXt / zTXt / iTXt chunks (a different metadata mechanism than JPEG EXIF/XMP).
Not rendered; plaintext in the file. Then verify it reads back."""
from PIL import Image
from PIL.PngImagePlugin import PngInfo

SRC = "dlp_img_base.jpg"
OUT = "dlp_img_5_pngtext.png"

PAYLOAD_LINES = [
    "SSN: 078-05-1120",
    "Visa 4111 1111 1111 1111",
    "AWS_KEY AKIAIOSFODNN7EXAMPLE",
    "AWS_SECRET wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    "Email john.public@example.com  Phone (555) 010-0142",
    "Passport X12345678  DOB 1985-07-14",
]
BLOB = "\n".join(PAYLOAD_LINES)
MARKERS = ["078-05-1120", "4111 1111 1111 1111", "AKIAIOSFODNN7EXAMPLE",
           "john.public@example.com", "X12345678"]

img = Image.open(SRC).convert("RGB")
meta = PngInfo()
meta.add_text("Description", BLOB)                       # tEXt
meta.add_text("Comment", BLOB, zip=True)                 # zTXt (compressed)
meta.add_itxt("Keywords", "SSN;PAN;AWS;PII;passport")    # iTXt (utf-8)
meta.add_text("Author", "John Q. Public (synthetic)")
img.save(OUT, pnginfo=meta)

# verify
back = Image.open(OUT)
text = " ".join(str(v) for v in back.text.values())
with open(OUT, "rb") as f:
    raw = f.read().decode("latin-1", "ignore")
print("Wrote", OUT)
print("chunks:", list(back.text.keys()))
print("markers via PIL text:", [m for m in MARKERS if m in text])
print("markers in raw bytes:", [m for m in MARKERS if m in raw])
