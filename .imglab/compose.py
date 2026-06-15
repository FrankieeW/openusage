"""Compose the three feature screenshots side-by-side into a single image.

Inputs:
  docs/hub-1.0.2.png           (800x1778)
  docs/settings-1.0.2.png      (800x1778)
  docs/env-page-rich-1.0.1.png (800x1546)

Output:
  docs/preview-1.0.2.png  (~1740x1200)
"""

from PIL import Image
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"

SOURCES = [
    DOCS / "hub-1.0.2.png",
    DOCS / "settings-1.0.2.png",
    DOCS / "env-page-rich-1.0.1.png",
]

TARGET_HEIGHT = 1200
GAP = 24
BG = (0, 0, 0, 0)  # transparent


def main() -> None:
    resized = []
    for src in SOURCES:
        img = Image.open(src).convert("RGBA")
        ratio = TARGET_HEIGHT / img.height
        new_w = round(img.width * ratio)
        resized.append(img.resize((new_w, TARGET_HEIGHT), Image.LANCZOS))

    total_w = sum(im.width for im in resized) + GAP * (len(resized) - 1)
    canvas = Image.new("RGBA", (total_w, TARGET_HEIGHT), BG)
    x = 0
    for im in resized:
        canvas.paste(im, (x, 0), im)
        x += im.width + GAP

    out = DOCS / "preview-1.0.2.png"
    canvas.save(out, optimize=True)
    print(f"Wrote {out.relative_to(ROOT)} ({canvas.size[0]}x{canvas.size[1]})")


if __name__ == "__main__":
    main()
