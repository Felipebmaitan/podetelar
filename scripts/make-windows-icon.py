"""Regenerate original MIT-licensed app artwork (requires Pillow)."""

from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent / "assets" / "windows"
root.mkdir(parents=True, exist_ok=True)
scale = 4
art = Image.new("RGBA", (256 * scale, 256 * scale))
draw = ImageDraw.Draw(art)


def box(bounds, radius, color):
    draw.rounded_rectangle(tuple(v * scale for v in bounds), radius * scale, fill=color)


box((8, 8, 248, 248), 52, "#142331")
box((40, 51, 216, 181), 20, "#45ddc1")
box((51, 62, 205, 170), 11, "#142331")
draw.polygon([(106 * scale, 84 * scale), (106 * scale, 149 * scale),
              (159 * scale, 116 * scale)], fill="#f0fffb")
box((117, 179, 139, 204), 4, "#45ddc1")
box((85, 200, 171, 212), 6, "#45ddc1")
icon = art.resize((256, 256), Image.Resampling.LANCZOS)
icon.save(root / "freedomstream.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
# egui consumes raw RGBA, so startup needs no image decoder or external file.
(root / "freedomstream-64.rgba").write_bytes(
    icon.resize((64, 64), Image.Resampling.LANCZOS).tobytes()
)
