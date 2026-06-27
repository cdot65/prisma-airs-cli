#!/usr/bin/env python3
"""Embed synthetic DLP test data as invisible (render-mode-3) text into a PDF.

Visually unchanged; text lives in the content stream and extracts as clear text
(e.g. via pdftotext). Each line is placed in an empty vertical gap on the page so
extraction never interleaves it with real text and every value stays contiguous.
All values are synthetic / reserved-for-testing.
"""
import io
import pdfplumber
from pypdf import PdfReader, PdfWriter
from reportlab.pdfgen import canvas

SRC = "Keychron_Q6_HE_User_Manual.pdf"
OUT = "Keychron_Q6_HE_User_Manual_DLP.pdf"
PAGE_W, PAGE_H = 612, 792

# Region of the page we are willing to use (top-origin coords, like pdfplumber).
REGION_TOP = 95           # below the running header
REGION_BOTTOM = PAGE_H - 95   # above the running footer
NEED = 16                 # vertical clearance required around an injected line
STEP = 22                 # spacing when a gap can hold several lines

# Synthetic payload (all values reserved-for-testing / documented examples).
PAYLOAD = [
    "SSN: 078-05-1120",
    "SSN: 219-09-9999",
    "Social Security Number: 457-55-5462",
    "SSN 123-45-6789",
    "Visa 4111 1111 1111 1111",
    "Visa 4012 8888 8888 1881",
    "Mastercard 5500 0000 0000 0004",
    "Amex 3782 822463 10005",
    "Discover 6011 1111 1111 1117",
    "Card 5555 5555 5555 4444 exp 12/29 cvv 123",
    "aws_access_key_id = AKIAIOSFODNN7EXAMPLE",
    "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    "AWS_SESSION_TOKEN=FwoGZXIvYXdzEXAMPLEsessiontoken1234567890abcdef",
    "github_pat = ghp_1234567890abcdefghijklmnopqrstuvwx12",
    "API_TOKEN=sk_live_51H8xZ2eZvKYlo2C0EXAMPLEtokenvalue00",
    "password = P@ssw0rd!Synthetic2026",
    "db_conn = postgres://admin:S3cr3tPass@db.internal:5432/prod",
    "Name: John Q. Public",
    "DOB: 1985-07-14",
    "Address: 123 Main Street, Springfield, IL 62704",
    "Name: Jane A. Doe   DOB 1990-02-28",
    "Address: 456 Oak Avenue, Apt 7B, Austin, TX 78701",
    "Phone: (555) 010-0142",
    "Mobile: 555-0173",
    "Email: john.public@example.com",
    "Email: jane.doe@example.org",
    "Contact: support@example.net  +1-555-0188",
    "Passport No: X12345678 (synthetic)",
    "Driver License: D1234567 IL",
    "IBAN: GB82 WEST 1234 5698 7654 32",
    "Routing 021000021 Account 000123456789",
]


def occupied_intervals(page):
    """Merged (top, bottom) bands occupied by real text, top-origin coords."""
    spans = sorted((w["top"], w["bottom"]) for w in page.extract_words())
    merged = []
    for top, bot in spans:
        if merged and top <= merged[-1][1] + 2:
            merged[-1][1] = max(merged[-1][1], bot)
        else:
            merged.append([top, bot])
    return merged


def free_slots(page):
    """Baselines (top-origin y) in empty gaps, each with >=NEED clearance."""
    occ = occupied_intervals(page)
    # Build free bands within the usable region.
    bands, cursor = [], REGION_TOP
    for top, bot in occ:
        if top > cursor:
            bands.append((cursor, top))
        cursor = max(cursor, bot)
    if cursor < REGION_BOTTOM:
        bands.append((cursor, REGION_BOTTOM))

    slots = []
    for lo, hi in bands:
        usable_lo, usable_hi = lo + NEED, hi - NEED
        y = usable_lo
        while y <= usable_hi:
            slots.append(y)
            y += STEP
    return slots


def main():
    # 1. Find empty slots per page.
    page_slots = []
    with pdfplumber.open(SRC) as pdf:
        for page in pdf.pages:
            page_slots.append(free_slots(page))

    # 2. Assign each payload line to a (page, baseline) slot.
    placements = {i: [] for i in range(len(page_slots))}
    queue = list(PAYLOAD)
    # round-robin across pages so data is spread, not clustered
    pi = 0
    guard = 0
    while queue and guard < 100000:
        guard += 1
        slots = page_slots[pi]
        if slots:
            top_y = slots.pop(0)
            placements[pi].append((top_y, queue.pop(0)))
        pi = (pi + 1) % len(page_slots)
    if queue:
        raise SystemExit(f"Not enough empty slots; {len(queue)} lines unplaced")

    # 3. Build the invisible overlay.
    reader = PdfReader(SRC)
    buf = io.BytesIO()
    c = canvas.Canvas(buf, pagesize=(PAGE_W, PAGE_H))
    for i in range(len(reader.pages)):
        for top_y, text in placements[i]:
            baseline = PAGE_H - top_y          # convert top-origin -> bottom-origin
            t = c.beginText(72, baseline)
            t.setTextRenderMode(3)             # 3 = invisible
            t.setFont("Helvetica", 8)
            t.textLine(text)
            c.drawText(t)
        c.showPage()
    c.save()
    buf.seek(0)
    overlay = PdfReader(buf)

    # 4. Merge and write.
    writer = PdfWriter()
    for i, page in enumerate(reader.pages):
        page.merge_page(overlay.pages[i])
        writer.add_page(page)
    with open(OUT, "wb") as f:
        writer.write(f)

    used_pages = sum(1 for i in placements if placements[i])
    print(f"Wrote {OUT}: {len(reader.pages)} pages, "
          f"{len(PAYLOAD)} invisible lines across {used_pages} pages")


if __name__ == "__main__":
    main()
