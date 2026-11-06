#!/usr/bin/env python3
"""Icon generator for Z-Engine.

Renders the desktop app icons (.icns, .ico, PNGs) and the favicon from one
master SVG: the pearl pet glowing on a midnight squircle. The pet uses the
same 32x32 geometry as `ui/src/components/pet/Pet.svelte`, scaled into the
512 canvas.
"""

import os
import shutil
import subprocess
from pathlib import Path
from PIL import Image

REPO_ROOT = Path(__file__).resolve().parent.parent
ICONS_DIR = REPO_ROOT / "crates/z-engine-gui/src-tauri/icons"
UI_PUBLIC_DIR = REPO_ROOT / "crates/z-engine-gui/ui/public"

# Master SVG Definition (512x512 canvas)
# Full-bleed midnight squircle (#202024 to #0C0C0E) with soft blue and lilac
# glows, and the pearl pet glowing at its centre. Keep LogoMark.svelte (the
# About page logo) drawing the same art.
MASTER_SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <!-- Midnight Canvas -->
    <linearGradient id="bg" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#202024" />
      <stop offset="50%" stop-color="#151518" />
      <stop offset="100%" stop-color="#0C0C0E" />
    </linearGradient>

    <!-- Blue and Lilac Glows Behind the Pet -->
    <radialGradient id="glow-sky" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#4F86FF" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#4F86FF" stop-opacity="0.18" />
      <stop offset="100%" stop-color="#4F86FF" stop-opacity="0" />
    </radialGradient>
    <radialGradient id="glow-lilac" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#7F5CFF" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#7F5CFF" stop-opacity="0.18" />
      <stop offset="100%" stop-color="#7F5CFF" stop-opacity="0" />
    </radialGradient>

    <!-- Lilac Light Pooled Under the Pet -->
    <radialGradient id="pool" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#9A84FF" stop-opacity="0.32" />
      <stop offset="60%" stop-color="#9A84FF" stop-opacity="0.08" />
      <stop offset="100%" stop-color="#9A84FF" stop-opacity="0" />
    </radialGradient>

    <!-- Contact Shadow Under the Feet -->
    <radialGradient id="contact" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#000000" stop-opacity="0.6" />
      <stop offset="60%" stop-color="#000000" stop-opacity="0.2" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0" />
    </radialGradient>

    <!-- Top Gloss and Bottom Falloff -->
    <linearGradient id="gloss" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="38%" stop-color="#FFFFFF" stop-opacity="0" />
      <stop offset="70%" stop-color="#000000" stop-opacity="0" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0.4" />
    </linearGradient>

    <!-- Squircle Specular Bevel (faint lit top edge, dark bottom edge) -->
    <linearGradient id="bevel" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.32" />
      <stop offset="14%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="60%" stop-color="#FFFFFF" stop-opacity="0" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0.5" />
    </linearGradient>

    <!-- Keeps the Glows Inside the Squircle -->
    <clipPath id="squircle"><rect width="512" height="512" rx="116" /></clipPath>

    <!-- Pearl Body -->
    <radialGradient id="body" cx="36%" cy="28%" r="82%">
      <stop offset="0%" stop-color="#FFFFFF" />
      <stop offset="42%" stop-color="#F2F4F9" />
      <stop offset="74%" stop-color="#D9DDE7" />
      <stop offset="100%" stop-color="#B3B8C8" />
    </radialGradient>

    <!-- Pearlescent Sheen (picks up the sky and lilac glows) -->
    <linearGradient id="sheen" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#bcd9ff" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#bcd9ff" stop-opacity="0" />
      <stop offset="60%" stop-color="#d9c8ff" stop-opacity="0" />
      <stop offset="100%" stop-color="#d9c8ff" stop-opacity="0.65" />
    </linearGradient>

    <!-- Inner Bloom (soft light pooled low in the body) -->
    <radialGradient id="bloom" cx="50%" cy="64%" r="46%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.35" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0" />
    </radialGradient>

    <!-- Feet: lit from above, shaded below -->
    <linearGradient id="foot" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#E1E4EC" />
      <stop offset="100%" stop-color="#B2B7C7" />
    </linearGradient>

    <!-- Blue Aura Around the Pet -->
    <filter id="aura" x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="3.0" />
    </filter>

    <!-- Drop Shadow for the Pet -->
    <filter id="pet-shadow" x="-30%" y="-30%" width="160%" height="170%">
      <feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000000" flood-opacity="0.55" />
      <feDropShadow dx="0" dy="3" stdDeviation="4" flood-color="#000000" flood-opacity="0.35" />
    </filter>
  </defs>

  <!-- Midnight Squircle Base -->
  <rect width="512" height="512" rx="116" fill="url(#bg)" />
  <g clip-path="url(#squircle)">
    <circle cx="200" cy="200" r="220" fill="url(#glow-sky)" />
    <circle cx="318" cy="318" r="220" fill="url(#glow-lilac)" />
    <ellipse cx="256" cy="404" rx="180" ry="36" fill="url(#pool)" />
  </g>

  <!-- Contact Shadow -->
  <ellipse cx="256" cy="402" rx="120" ry="16" fill="url(#contact)" />

  <!-- Pet Aura -->
  <g transform="translate(16 -28) scale(15)">
    <path d="M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z" fill="#9DB8FF" opacity="0.55" filter="url(#aura)" />
  </g>

  <!-- Pearl Pet (Pet.svelte 32x32 geometry, scaled 15x) -->
  <g filter="url(#pet-shadow)">
    <g transform="translate(16 -28) scale(15)">
      <ellipse cx="11.8" cy="27.2" rx="2.6" ry="1.5" fill="url(#foot)" />
      <ellipse cx="20.2" cy="27.2" rx="2.6" ry="1.5" fill="url(#foot)" />
      <path d="M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z" fill="url(#body)" />
      <path d="M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z" fill="url(#bloom)" />
      <path d="M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z" fill="url(#sheen)" />
      <path d="M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z" fill="none" stroke="#FFFFFF" stroke-opacity="0.6" stroke-width="0.4" />
      <ellipse cx="11.8" cy="12.9" rx="3.4" ry="1.9" transform="rotate(-28 11.8 12.9)" fill="#FFFFFF" opacity="0.92" />
      <ellipse cx="9.3" cy="21.7" rx="1.9" ry="1.1" fill="#ff8fa3" opacity="0.45" />
      <ellipse cx="22.7" cy="21.7" rx="1.9" ry="1.1" fill="#ff8fa3" opacity="0.45" />
      <ellipse cx="12.4" cy="18.4" rx="1.35" ry="2" fill="#14151a" />
      <ellipse cx="19.6" cy="18.4" rx="1.35" ry="2" fill="#14151a" />
      <path d="M14.6 22.5q1.4 1.3 2.8 0" fill="none" stroke="#14151a" stroke-width="0.9" stroke-linecap="round" />
    </g>
  </g>

  <!-- Top Gloss and Bottom Falloff -->
  <rect width="512" height="512" rx="116" fill="url(#gloss)" />

  <!-- Specular Bevel -->
  <rect x="1.5" y="1.5" width="509" height="509" rx="114.5" fill="none" stroke="url(#bevel)" stroke-width="3" />
  <rect x="0.5" y="0.5" width="511" height="511" rx="115.5" fill="none" stroke="#FFFFFF" stroke-opacity="0.12" stroke-width="1" />
</svg>
"""


def render_svg_to_png(svg_path: Path, png_path: Path, width: int, height: int):
    """Render SVG to raster PNG using rsvg-convert."""
    subprocess.run(
        ["rsvg-convert", "-w", str(width), "-h", str(height), str(svg_path), "-o", str(png_path)],
        check=True,
    )


def generate_icns(master_png: Path, icns_path: Path):
    """Build macOS .icns using iconutil."""
    tmp_iconset = icns_path.parent / "tmp.iconset"
    if tmp_iconset.exists():
        shutil.rmtree(tmp_iconset)
    tmp_iconset.mkdir(parents=True)

    iconset_specs = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    master = Image.open(master_png)
    for filename, size in iconset_specs:
        resized = master.resize((size, size), Image.Resampling.LANCZOS)
        resized.save(tmp_iconset / filename)

    subprocess.run(["iconutil", "-c", "icns", str(tmp_iconset), "-o", str(icns_path)], check=True)
    shutil.rmtree(tmp_iconset)


def generate_ico(master_png: Path, ico_path: Path):
    """Build multi-resolution Windows .ico file."""
    master = Image.open(master_png)
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    master.save(str(ico_path), format="ICO", sizes=sizes)


def main():
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    UI_PUBLIC_DIR.mkdir(parents=True, exist_ok=True)

    # 1. Write SVG assets
    svg_icon_path = ICONS_DIR / "icon.svg"
    favicon_path = UI_PUBLIC_DIR / "favicon.svg"

    svg_icon_path.write_text(MASTER_SVG, encoding="utf-8")
    favicon_path.write_text(MASTER_SVG, encoding="utf-8")
    print(f"Wrote {svg_icon_path} and {favicon_path}")

    # 2. Render 1024x1024 master raster PNG
    master_png_path = ICONS_DIR / "icon-1024.png"
    render_svg_to_png(svg_icon_path, master_png_path, 1024, 1024)

    # 3. Generate standard PNG sizes
    master_img = Image.open(master_png_path)
    target_sizes = {
        "icon.png": (512, 512),
        "icon-512.png": (512, 512),
        "icon-256.png": (256, 256),
        "128x128.png": (128, 128),
        "128x128@2x.png": (256, 256),
        "64x64.png": (64, 64),
        "32x32.png": (32, 32),
        # Windows Appx Logos
        "StoreLogo.png": (50, 50),
        "Square30x30Logo.png": (30, 30),
        "Square44x44Logo.png": (44, 44),
        "Square71x71Logo.png": (71, 71),
        "Square89x89Logo.png": (89, 89),
        "Square107x107Logo.png": (107, 107),
        "Square142x142Logo.png": (142, 142),
        "Square150x150Logo.png": (150, 150),
        "Square284x284Logo.png": (284, 284),
        "Square310x310Logo.png": (310, 310),
    }

    for fname, size in target_sizes.items():
        out_file = ICONS_DIR / fname
        resized = master_img.resize(size, Image.Resampling.LANCZOS)
        resized.save(out_file)
        print(f"Generated {out_file.name} ({size[0]}x{size[1]})")

    # 4. Generate native macOS icon.icns
    icns_path = ICONS_DIR / "icon.icns"
    generate_icns(master_png_path, icns_path)
    print(f"Generated {icns_path.name}")

    # 5. Generate Windows icon.ico
    ico_path = ICONS_DIR / "icon.ico"
    generate_ico(master_png_path, ico_path)
    print(f"Generated {ico_path.name}")

    # Remove temporary 1024 png if not strictly required by Tauri
    if master_png_path.exists():
        master_png_path.unlink()

    print("Procedural icon generation completed successfully!")


if __name__ == "__main__":
    main()
