# Test File Catalog

Per-file detail: what each carrier is, what is embedded, where/how it is hidden, and how a
scanner would have to detect it. All payloads are the synthetic markers documented on the
[overview](index.md).

Each entry links to the raw carrier (`samples/`) and its base64 encoding (`encoded/`).

---

## PDF

### `Keychron_Q6_HE_User_Manual_DLP.pdf`

- **Source:** [samples/Keychron_Q6_HE_User_Manual_DLP.pdf](samples/Keychron_Q6_HE_User_Manual_DLP.pdf) · [base64](encoded/Keychron_Q6_HE_User_Manual_DLP.pdf.b64)
- **Technique:** invisible text layer using PDF **text render mode 3** (the same mechanism OCR
  layers use). 31 synthetic lines placed in empty vertical gaps across 18 of 22 pages.
- **Within it:** SSNs, credit-card PANs, AWS keys/tokens, a password, a DB connection string,
  synthetic identities (names/addresses/DOB), emails, phones, passport/DL/IBAN/routing.
- **Visible?** No — pages render pixel-identical to the original.
- **Detect by:** PDF text extraction (`pdftotext`); each value extracts contiguously.
- **Result:** :material-check: **detected** by the scanner.
- **Generator:** `scripts/embed_dlp.py`

---

## Image ladder

A single base image carried into four layers, each probing a different scanner capability.

### `dlp_img_base.jpg`

- **Source:** [samples/dlp_img_base.jpg](samples/dlp_img_base.jpg)
- The clean synthetic base image (gradient + colored blocks). **No payload.** All other image
  files derive from this.

### `dlp_img_1_metadata.jpg` — EXIF + XMP

- **Source:** [samples/dlp_img_1_metadata.jpg](samples/dlp_img_1_metadata.jpg) · [base64](encoded/dlp_img_1_metadata.jpg.b64)
- **Within it:** markers in EXIF `ImageDescription`, `Artist`, `Copyright`, `XPComment`,
  `XPKeywords`, `UserComment`, plus an XMP packet (`dc:description`, `dc:subject`).
- **Visible?** No (metadata, not rendered).
- **Detect by:** parsing image metadata.
- **Result:** :material-close: not detected.

### `dlp_img_2_container.jpg` — container plaintext

- **Source:** [samples/dlp_img_2_container.jpg](samples/dlp_img_2_container.jpg) · [base64](encoded/dlp_img_2_container.jpg.b64)
- **Within it:** a JPEG `COM` comment segment (after `SOI`) **and** plaintext bytes appended
  after the `FFD9` end-of-image marker (ignored by viewers).
- **Visible?** No.
- **Detect by:** raw whole-file/byte scanning, not just recognized fields.
- **Result:** :material-close: not detected.

### `dlp_img_3_ocr.jpg` — rendered pixels (OCR)

- **Source:** [samples/dlp_img_3_ocr.jpg](samples/dlp_img_3_ocr.jpg) · [base64](encoded/dlp_img_3_ocr.jpg.b64)
- **Within it:** the markers **painted onto the image** as actual pixels (dark text on a light
  band). There is **no text layer** — the data exists only as pixels.
- **Visible?** **Yes.**
- **Detect by:** running OCR on the image (verified recoverable with `tesseract`).
- **Result:** :material-close: not detected — the scanner does not OCR.

### `dlp_img_4_stego.png` — LSB steganography

- **Source:** [samples/dlp_img_4_stego.png](samples/dlp_img_4_stego.png) · [base64](encoded/dlp_img_4_stego.png.b64)
- **Within it:** markers encoded into the **least-significant bits** of pixel data, with a
  4-byte length prefix. Decoded payload is 209 bytes.
- **Why PNG (not JPEG):** classic LSB steg does **not** survive JPEG's lossy DCT
  quantization; a lossless format is required. True in-JPEG steg needs DCT-coefficient
  embedding (steghide/F5-style).
- **Visible?** No (invisible; not plaintext anywhere in the file).
- **Detect by:** steganalysis / LSB extraction.
- **Result:** :material-alert: flagged as **"toxic content."** Because the plaintext PII in
  #1–#3 was missed, this most likely indicates **steg-presence detection**, not payload
  reading. See controls below.

---

## Controls (disambiguate the #4 result)

### `dlp_ctrl_clean.png`

- **Source:** [samples/dlp_ctrl_clean.png](samples/dlp_ctrl_clean.png) · [base64](encoded/dlp_ctrl_clean.png.b64)
- Same image saved as PNG with **no embedded data**. False-positive control: if this flags,
  the trigger is the image itself, not hidden data.

### `dlp_ctrl_stego_benign.png`

- **Source:** [samples/dlp_ctrl_stego_benign.png](samples/dlp_ctrl_stego_benign.png) · [base64](encoded/dlp_ctrl_stego_benign.png.b64)
- LSB steg carrying **only benign lorem-ipsum** (verified to contain no markers). Isolates
  *steg-presence* from *payload content*: if this still flags "toxic," the scanner is
  reacting to steganography, not to the sensitive data.

---

## Other modalities

### `dlp_doc_sensitive.docx`

- **Source:** [samples/dlp_doc_sensitive.docx](samples/dlp_doc_sensitive.docx) · [base64](encoded/dlp_doc_sensitive.docx.b64)
- **Within it:** markers in the **visible body**, a **hidden run** (white, 1pt font), and the
  **core document properties** (`author`, `comments`, `keywords`).
- **Detect by:** Office Open XML parsing — body text, run-level formatting, and `docProps`.

### `dlp_archive.zip`

- **Source:** [samples/dlp_archive.zip](samples/dlp_archive.zip) · [base64](encoded/dlp_archive.zip.b64)
- **Within it:** `payload.txt` (the markers as plaintext) compressed inside the archive.
- **Detect by:** archive recursion — decompress and scan contained files.

### `payload.txt`

- **Source:** [samples/payload.txt](samples/payload.txt)
- The raw synthetic payload as plaintext. Baseline: a scanner that misses this misses
  everything.

---

## Metadata variants

### `dlp_img_5_pngtext.png` — PNG text chunks

- **Source:** [samples/dlp_img_5_pngtext.png](samples/dlp_img_5_pngtext.png) · [base64](encoded/dlp_img_5_pngtext.png.b64)
- **Within it:** markers in PNG `tEXt` / `zTXt` (compressed) / `iTXt` (utf-8) chunks — a
  different metadata mechanism than JPEG EXIF/XMP. Tests whether a metadata blind spot
  extends to PNG textual metadata.

### `dlp_img_6_iptc.jpg` — IPTC

- **Source:** [samples/dlp_img_6_iptc.jpg](samples/dlp_img_6_iptc.jpg) · [base64](encoded/dlp_img_6_iptc.jpg.b64)
- **Within it:** markers across IPTC IIM fields (`Caption-Abstract`, `Headline`, `Keywords`,
  `By-line`, `SpecialInstructions`, `Credit`, `Source`). IPTC is the metadata standard most
  asset-management and many DLP tools read.
