"""Derive Windows/native icons from assets/PodeTelar.png (requires Pillow)."""

from pathlib import Path
from PIL import Image

assets = Path(__file__).resolve().parent.parent / "assets"
output = assets / "windows"
output.mkdir(parents=True, exist_ok=True)

# Preserve the supplied artwork and colors; only resize it for each surface.
with Image.open(assets / "PodeTelar.png") as source:
    artwork = source.convert("RGBA")
    if artwork.width != artwork.height:
        raise ValueError("The PodeTelar icon must be square.")
    icon = artwork.resize((256, 256), Image.Resampling.LANCZOS)
    icon.save(
        output / "podetelar.ico",
        sizes=[(size, size) for size in (16, 24, 32, 48, 64, 128, 256)],
    )
    # egui consumes raw RGBA, so startup needs no decoder or external file.
    (output / "podetelar-64.rgba").write_bytes(
        artwork.resize((64, 64), Image.Resampling.LANCZOS).tobytes()
    )
