# DLP Detection Testing

A corpus of crafted files used to evaluate how well a content scanner (e.g. Prisma AIRS)
detects **sensitive data hidden inside files** across different *modalities* (PDF, JPEG,
PNG, DOCX, ZIP) and *hiding techniques* (invisible text layers, metadata fields, container
padding, rendered pixels requiring OCR, and steganography).

Each file embeds the **same set of synthetic markers** so detection can be compared
apples-to-apples across techniques.

!!! danger "All data is synthetic — no real PII"
    Every value in this corpus is drawn from a reserved / documented test range and refers
    to no real person or account:

    | Type | Value | Why it's safe |
    | --- | --- | --- |
    | SSN | `078-05-1120` | Historically reserved demo SSN, never issued |
    | Credit card | `4111 1111 1111 1111` | Standard Visa test PAN (passes Luhn, not a real account) |
    | AWS credentials | `AKIAIOSFODNN7EXAMPLE` / `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` | AWS's own documented example key/secret |
    | Email | `john.public@example.com` | IANA-reserved `example.com` domain |
    | Phone | `(555) 010-0142` | `555-0100`–`555-0199` fictional-use block |
    | Identity | `Passport X12345678`, `DOB 1985-07-14` | Invented |

## Methodology

1. **Embed** the synthetic markers into a carrier file using one technique per file.
2. Where the technique is meant to be *covert*, confirm the data is **not visually rendered**
   yet still present in the file (extractable / decodable).
3. **Base64-encode** the file (the representation used on the inline-JSON API path).
4. **Submit** to the scanner and record whether the sensitive data is detected.

See [Test File Catalog](catalog.md) for exactly what each file contains and how the data is hidden.

## Results so far

Legend: :material-check: detected · :material-close: not detected · :material-alert: anomalous · — untested

| Modality / technique | File | Visible? | Detected? | Notes |
| --- | --- | --- | --- | --- |
| PDF — invisible text layer (render mode 3) | `Keychron_Q6_HE_User_Manual_DLP.pdf` | No | :material-check: | 31 lines across 18 pages; **caught** |
| JPEG — EXIF + XMP metadata | `dlp_img_1_metadata.jpg` | No | :material-close: | metadata not parsed |
| JPEG — COM segment + bytes after EOI | `dlp_img_2_container.jpg` | No | :material-close: | raw container not scanned |
| JPEG — rendered pixels (OCR needed) | `dlp_img_3_ocr.jpg` | **Yes** | :material-close: | scanner does not OCR |
| PNG — LSB steganography | `dlp_img_4_stego.png` | No | :material-alert: | flagged **"toxic content"** — see below |
| JPEG — IPTC metadata | `dlp_img_6_iptc.jpg` | No | — | metadata variant |
| PNG — text chunks (tEXt/zTXt/iTXt) | `dlp_img_5_pngtext.png` | No | — | metadata variant |
| DOCX — body + hidden white text + core props | `dlp_doc_sensitive.docx` | Partly | — | Office modality |
| ZIP — payload.txt inside archive | `dlp_archive.zip` | No | — | archive recursion |
| Plaintext baseline | `samples/payload.txt` | Yes | — | sanity baseline |
| SVG — benign controls | `samples/svg/svg_benign_*.svg` | Yes | n/a | correctly **allowed** (true negatives) |
| SVG — DLP (sensitive data) | `samples/svg/svg_mal_1_dlp.svg` | No | :material-close: | **DLP bypass** — see [SVG DLP bypass finding](svg-dlp-bypass.md) |
| SVG — prompt injection | `samples/svg/svg_mal_2_prompt_injection.svg` | No | :material-check: | blocked (`injection`) |
| SVG — system-prompt extraction | `samples/svg/svg_mal_3_system_prompt.svg` | No | :material-check: | blocked (`injection`) |
| SVG — indirect injection + exfil | `samples/svg/svg_mal_4_exfil_injection.svg` | No | :material-check: | blocked (`injection`, `toxic_content`) |
| SVG — active content / script (XSS) | `samples/svg/svg_mal_5_script_xss.svg` | No | :material-check: | blocked (`injection`, `toxic_content`) — not `malicious_code` |

!!! warning "Open question — the stego PNG result"
    The plaintext PII in files #1–#3 was **missed**, but the *steganographic* PNG (#4) was
    flagged as **"toxic content."** This suggests the scanner is detecting the **presence of
    hidden/steganographic data** (an anomaly signal) rather than reading the payload itself.
    Two controls are included to confirm:

    - `dlp_ctrl_clean.png` — identical image, **no** embedded data (false-positive control).
    - `dlp_ctrl_stego_benign.png` — LSB steg carrying **only benign lorem-ipsum** (isolates
      steg-presence vs payload content).

    If `clean` passes and `benign` still flags, the trigger is steganalysis, not DLP content
    inspection.

## Layout

```
docs/dlp-detection/
├── index.md            # this page
├── catalog.md          # per-file detail: what is what, what is within what
├── samples/            # the raw carrier files (+ samples/svg/ for the SVG set)
├── encoded/            # base64 encodings (+ encoded/svg/)
└── scripts/            # generators + verifier (provenance / regenerate)
```

## Regenerate

From the `scripts/` directory (requires `pypdf reportlab pillow numpy piexif python-docx`,
plus `tesseract` and `exiftool` for the OCR/IPTC steps):

```bash
python3 embed_dlp.py        # the PDF invisible-text-layer set
python3 build_image_dlp.py  # image ladder: metadata / container / OCR / LSB stego
python3 build_png_text.py   # PNG text-chunk metadata variant
python3 build_more_dlp.py   # controls + DOCX + ZIP
python3 build_svg_corpus.py # SVG set: 2 benign + 5 malicious (DLP + AI-prompt attacks)
python3 verify_image_dlp.py # confirms each image still carries its payload
```

## Submit to a scanner

The `encoded/` files are ready for the inline-JSON path. Copy one to the clipboard:

```bash
pbcopy < encoded/dlp_img_4_stego.png.b64
```

Mind the media type per file: `application/pdf`, `image/jpeg`, `image/png`,
`application/vnd.openxmlformats-officedocument.wordprocessingml.document` (docx),
`application/zip`.
