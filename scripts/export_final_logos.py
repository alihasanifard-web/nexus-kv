# -*- coding: utf-8 -*-
"""
Render chosen SVGs to PNG & ICO using Headless Edge + Pillow.
Chosen concepts:
1. Server: server-concept-1-raft-triad.svg
2. CLI: cli-concept-2-prompt-cursor.svg
3. Bench: bench-concept-1-speed-arc.svg
4. Cockpit: cockpit-concept-1-radar-scope.svg
"""

import os
import shutil
import subprocess
from PIL import Image

EDGE_PATH = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
SCRATCH_DIR = r"C:\Users\V I S I O N\AppData\Local\hermes\cache\scratch"
PROJECT_DIR = r"C:\Users\V I S I O N\nexus-kv"
ASSETS_DIR = os.path.join(PROJECT_DIR, "assets")
EXPORT_DIR = os.path.join(PROJECT_DIR, "logos", "export")

os.makedirs(ASSETS_DIR, exist_ok=True)
os.makedirs(EXPORT_DIR, exist_ok=True)

chosen = {
    "server": {
        "src": os.path.join(PROJECT_DIR, "logos", "concepts", "server-concept-1-raft-triad.svg"),
        "ico": os.path.join(ASSETS_DIR, "server.ico"),
        "png": os.path.join(ASSETS_DIR, "server.png"),
        "export_svg": os.path.join(EXPORT_DIR, "server-logo.svg")
    },
    "cli": {
        "src": os.path.join(PROJECT_DIR, "logos", "concepts", "cli-concept-2-prompt-cursor.svg"),
        "ico": os.path.join(ASSETS_DIR, "cli.ico"),
        "png": os.path.join(ASSETS_DIR, "cli.png"),
        "export_svg": os.path.join(EXPORT_DIR, "cli-logo.svg")
    },
    "bench": {
        "src": os.path.join(PROJECT_DIR, "logos", "concepts", "bench-concept-1-speed-arc.svg"),
        "ico": os.path.join(ASSETS_DIR, "bench.ico"),
        "png": os.path.join(ASSETS_DIR, "bench.png"),
        "export_svg": os.path.join(EXPORT_DIR, "bench-logo.svg")
    },
    "cockpit": {
        "src": os.path.join(PROJECT_DIR, "logos", "concepts", "cockpit-concept-1-radar-scope.svg"),
        "ico": os.path.join(ASSETS_DIR, "cockpit.ico"),
        "png": os.path.join(ASSETS_DIR, "cockpit.png"),
        "export_svg": os.path.join(EXPORT_DIR, "cockpit-logo.svg")
    }
}

for name, info in chosen.items():
    shutil.copyfile(info["src"], info["export_svg"])
    
    # Read SVG
    with open(info["src"], "r", encoding="utf-8") as f:
        svg_content = f.read()

    # Create temporary standalone HTML with pure dark background for clean rendering
    html_temp = os.path.join(SCRATCH_DIR, f"{name}_render.html")
    with open(html_temp, "w", encoding="utf-8") as f:
        f.write(f"""<!DOCTYPE html>
<html>
<head><style>
  html, body {{ margin:0; padding:0; background:#00000000; overflow:hidden; width:512px; height:512px; }}
  svg {{ width:100%; height:100%; }}
</style></head>
<body>
{svg_content}
</body>
</html>""")

    png_temp = os.path.join(SCRATCH_DIR, f"{name}_512.png")
    
    # Render with Edge headless at 512x512
    cmd = [
        EDGE_PATH,
        "--headless",
        f"--screenshot={png_temp}",
        "--window-size=512,512",
        "--default-background-color=00000000",
        f"file:///{html_temp.replace(os.sep, '/')}"
    ]
    subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    
    if os.path.exists(png_temp):
        im = Image.open(png_temp).convert("RGBA")
        im.save(info["png"], format="PNG")
        
        # Create multi-size ICO
        sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)]
        im.save(info["ico"], format="ICO", sizes=sizes)
        print(f"[OK] Generated {name}.ico and {name}.png ({os.path.getsize(info['ico'])} bytes)")
    else:
        print(f"[FAIL] Could not render {name}")

print("All icons successfully generated!")
