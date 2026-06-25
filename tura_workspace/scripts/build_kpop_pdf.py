from pathlib import Path
from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "output"
OUT_DIR.mkdir(exist_ok=True)

PAGE_W, PAGE_H = 2480, 3508
BG = (18, 14, 17)
INK = (245, 238, 241)
MUTED = (174, 151, 161)
PINK = (215, 132, 159)
DARK_PINK = (92, 42, 58)
LINE = (76, 56, 65)


def font(size, bold=False):
    candidates = [
        r"C:\Windows\Fonts\arialbd.ttf" if bold else r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\seguisb.ttf" if bold else r"C:\Windows\Fonts\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf" if bold else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    for p in candidates:
        if Path(p).exists():
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


F_TITLE = font(168, True)
F_H1 = font(112, True)
F_H2 = font(58, True)
F_BODY = font(44)
F_SMALL = font(31)
F_TINY = font(25)


MEMBERS = [
    {
        "name": "MINA",
        "role": "Leader / Main Vocal",
        "tag": "velvet choker + cold doll gaze",
        "file": ROOT / "media/members/mina/generate-media-replicate_z_image_turbo-1.png",
        "notes": ["black velvet O-ring choker", "tailored harness jacket", "soft pink under-eye blush"],
    },
    {
        "name": "RINA",
        "role": "Center / Visual",
        "tag": "ribbon choker + sweet menace",
        "file": ROOT / "media/members/rina/generate-media-replicate_z_image_turbo-1.png",
        "notes": ["heart-ring satin choker", "lace blouse and corset line", "ash-pink twin-tail silhouette"],
    },
    {
        "name": "SEO",
        "role": "Main Rapper / Performance",
        "tag": "patent choker + silver hardware",
        "file": ROOT / "media/members/seo/generate-media-replicate_z_image_turbo-1.png",
        "notes": ["patent leather collar", "chain and shoulder straps", "silver-lavender hair contrast"],
    },
    {
        "name": "YUNA",
        "role": "Lead Vocal / Moodmaker",
        "tag": "frill choker + fragile charm",
        "file": ROOT / "media/members/yuna/generate-media-replicate_z_image_turbo-1.png",
        "notes": ["layered ribbon choker", "black frilled dress", "glossy tear-line makeup"],
    },
    {
        "name": "NARI",
        "role": "Main Dancer / Maknae",
        "tag": "buckle choker + sharp stage bite",
        "file": ROOT / "media/members/nari/generate-media-replicate_z_image_turbo-1.png",
        "notes": ["wide leather buckle choker", "asymmetric harness top", "midnight-blue wolf cut"],
    },
]


def page():
    im = Image.new("RGB", (PAGE_W, PAGE_H), BG)
    draw = ImageDraw.Draw(im)
    for y in range(PAGE_H):
        mix = y / PAGE_H
        r = int(BG[0] * (1 - mix) + 34 * mix)
        g = int(BG[1] * (1 - mix) + 20 * mix)
        b = int(BG[2] * (1 - mix) + 27 * mix)
        draw.line([(0, y), (PAGE_W, y)], fill=(r, g, b))
    return im


def fit_crop(img, box):
    x, y, w, h = box
    src = img.convert("RGB")
    scale = max(w / src.width, h / src.height)
    nw, nh = int(src.width * scale), int(src.height * scale)
    src = src.resize((nw, nh), Image.Resampling.LANCZOS)
    left, top = (nw - w) // 2, (nh - h) // 2
    crop = src.crop((left, top, left + w, top + h))
    mask = Image.new("L", (w, h), 255)
    return crop, mask


def text(draw, xy, s, f, fill=INK, anchor=None):
    draw.text(xy, s, font=f, fill=fill, anchor=anchor)


def wrap(draw, s, f, max_w):
    words = s.split()
    lines, cur = [], ""
    for word in words:
        test = (cur + " " + word).strip()
        if draw.textbbox((0, 0), test, font=f)[2] <= max_w:
            cur = test
        else:
            if cur:
                lines.append(cur)
            cur = word
    if cur:
        lines.append(cur)
    return lines


def pill(draw, xy, label):
    x, y = xy
    bbox = draw.textbbox((0, 0), label, font=F_SMALL)
    w, h = bbox[2] + 46, bbox[3] + 28
    draw.rounded_rectangle((x, y, x + w, y + h), radius=6, fill=(42, 29, 36), outline=LINE, width=2)
    text(draw, (x + 23, y + 13), label, F_SMALL, MUTED)
    return w


def add_footer(draw, idx):
    draw.line((170, PAGE_H - 165, PAGE_W - 170, PAGE_H - 165), fill=LINE, width=2)
    text(draw, (170, PAGE_H - 118), "FICTIONAL K-POP CONCEPT BOOK", F_TINY, MUTED)
    text(draw, (PAGE_W - 170, PAGE_H - 118), f"{idx:02d}", F_TINY, MUTED, anchor="ra")


def cover():
    im = page(); d = ImageDraw.Draw(im)
    imgs = [Image.open(m["file"]) for m in MEMBERS]
    boxes = [(130, 330, 650, 870), (785, 720, 440, 610), (1245, 260, 610, 830), (1820, 850, 480, 660)]
    for img, box in zip([imgs[0], imgs[1], imgs[2], imgs[4]], boxes):
        crop, mask = fit_crop(img, box)
        im.paste(crop, box[:2], mask)
        d.rectangle((box[0], box[1], box[0]+box[2], box[1]+box[3]), outline=LINE, width=5)
    d.rectangle((150, 1970, PAGE_W-150, 2860), fill=(21, 15, 19))
    text(d, (190, 2045), "VELVET COLLAR", F_TITLE, INK)
    text(d, (198, 2220), "CHOKER / STAGE BONDAGE ELEMENTS / JIRAI-KEI", F_H2, PINK)
    body = "A five-member fictional K-pop concept built around black velvet, metal hardware, ribbon chokers, doll-like under-eye makeup, and controlled stage tension. Dark, cute, precise. Annoyingly effective."
    y = 2365
    for line in wrap(d, body, F_BODY, 1670):
        text(d, (200, y), line, F_BODY, MUTED); y += 64
    x = 200
    for label in ["BLACK", "DUSTY ROSE", "SILVER", "LACE", "LEATHER"]:
        x += pill(d, (x, 2665), label) + 20
    add_footer(d, 1)
    return im


def concept_page():
    im = page(); d = ImageDraw.Draw(im)
    text(d, (170, 260), "CONCEPT DNA", F_H1, INK)
    text(d, (170, 382), "sweet surface, controlled danger, stage-safe restraint", F_H2, PINK)
    sections = [
        ("Palette", "Black as the base, dusty rose as the emotional signal, silver hardware as the cold edge."),
        ("Styling", "Chokers lead every look. SM-inspired elements stay in fashion language: buckles, straps, rings, lace gloves, corset seams."),
        ("Makeup", "Jirai-kei influence through doll eyes, soft under-eye blush, glossy lips, pale skin contrast, and a fragile-cute mood."),
        ("Stage mood", "Minimal set, hard spotlight, slow choreography accents, close camera cuts on collar hardware and eye makeup."),
    ]
    y = 690
    for title, body in sections:
        d.rectangle((170, y, PAGE_W-170, y+410), outline=LINE, width=3)
        text(d, (230, y+70), title.upper(), F_H2, INK)
        yy = y + 160
        for line in wrap(d, body, F_BODY, 1750):
            text(d, (230, yy), line, F_BODY, MUTED); yy += 62
        y += 505
    add_footer(d, 2)
    return im


def group_page():
    im = page(); d = ImageDraw.Draw(im)
    text(d, (170, 240), "MEMBER LINE", F_H1, INK)
    text(d, (170, 360), "five silhouettes, one collar language", F_H2, PINK)
    y = 610
    thumb_w, thumb_h = 350, 467
    for i, m in enumerate(MEMBERS):
        img = Image.open(m["file"])
        x = 170 + i * 455
        crop, mask = fit_crop(img, (x, y, thumb_w, thumb_h))
        im.paste(crop, (x, y), mask)
        d.rectangle((x, y, x+thumb_w, y+thumb_h), outline=LINE, width=4)
        text(d, (x, y+thumb_h+60), m["name"], F_H2, INK)
        text(d, (x, y+thumb_h+118), m["role"], F_SMALL, MUTED)
    d.rectangle((170, 1970, PAGE_W-170, 2760), fill=(24, 16, 22), outline=LINE, width=3)
    text(d, (240, 2060), "GROUP PROFILE", F_H2, INK)
    copy = "VELVET COLLAR is a fictional five-member K-pop group concept. The visual system uses choker details as the signature, SM-coded hardware as stage fashion, and jirai-kei softness to keep the mood pretty but unstable."
    yy = 2170
    for line in wrap(d, copy, F_BODY, 1700):
        text(d, (240, yy), line, F_BODY, MUTED); yy += 66
    add_footer(d, 3)
    return im


def member_page(m, idx):
    im = page(); d = ImageDraw.Draw(im)
    img = Image.open(m["file"])
    crop, mask = fit_crop(img, (140, 230, 1180, 1570))
    im.paste(crop, (140, 230), mask)
    d.rectangle((140, 230, 1320, 1800), outline=LINE, width=5)
    text(d, (1440, 370), m["name"], F_TITLE, INK)
    text(d, (1450, 555), m["role"], F_H2, PINK)
    text(d, (1450, 675), m["tag"], F_BODY, MUTED)
    d.line((1450, 820, PAGE_W-180, 820), fill=LINE, width=3)
    y = 945
    for note in m["notes"]:
        d.ellipse((1450, y+14, 1480, y+44), fill=PINK)
        text(d, (1515, y), note, F_BODY, INK)
        y += 110
    d.rectangle((140, 2070, PAGE_W-140, 2920), fill=(24, 17, 22), outline=LINE, width=3)
    text(d, (215, 2160), "VISUAL DIRECTION", F_H2, INK)
    copy = f"{m['name']} carries the concept through collar-first styling: the neck detail is the anchor, while lace, straps, and metal accents build a controlled stage silhouette. The makeup keeps the jirai-kei fragility visible without softening the darker K-pop edge."
    yy = 2280
    for line in wrap(d, copy, F_BODY, 1870):
        text(d, (215, yy), line, F_BODY, MUTED); yy += 66
    add_footer(d, idx)
    return im


def main():
    for m in MEMBERS:
        if not m["file"].exists():
            raise FileNotFoundError(m["file"])
    pages = [cover(), concept_page(), group_page()]
    pages += [member_page(m, i + 4) for i, m in enumerate(MEMBERS)]
    pdf = OUT_DIR / "velvet-collar-kpop-concept.pdf"
    png = OUT_DIR / "velvet-collar-cover-preview.png"
    sheet = OUT_DIR / "velvet-collar-page-check.png"
    pages[0].save(png)
    thumb_w = 310
    thumb_h = int(thumb_w * PAGE_H / PAGE_W)
    contact = Image.new("RGB", (thumb_w * 4 + 90, thumb_h * 2 + 90), (18, 14, 17))
    cd = ImageDraw.Draw(contact)
    for i, p in enumerate(pages):
        thumb = p.resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)
        x = 30 + (i % 4) * (thumb_w + 10)
        y = 30 + (i // 4) * (thumb_h + 35)
        contact.paste(thumb, (x, y))
        cd.text((x, y + thumb_h + 6), f"PAGE {i + 1}", font=F_TINY, fill=MUTED)
    contact.save(sheet)
    pages[0].save(pdf, save_all=True, append_images=pages[1:], resolution=180.0)
    print(pdf)
    print(png)
    print(sheet)


if __name__ == "__main__":
    main()
