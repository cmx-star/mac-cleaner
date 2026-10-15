"""生成全套 Tauri 图标：PNG 各尺寸 + icns + ico。"""
import subprocess, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from gen_icon import render

ICONSET = "src-tauri/icons/icon.iconset"
SIZES = {
    16: "icon_16x16.png",
    32: "icon_16x16@2x.png",
    32+0: "icon_32x32.png",
    64: "icon_32x32@2x.png",
    128: "icon_128x128.png",
    256: "icon_128x128@2x.png",
    256+0: "icon_256x256.png",
    512: "icon_256x256@2x.png",
    512+0: "icon_512x512.png",
}
os.makedirs(ICONSET, exist_ok=True)
for sz, name in SIZES.items():
    render(sz).save(os.path.join(ICONSET, name))
    print("png", sz, name)

# 主尺寸
render(32).save("src-tauri/icons/32x32.png")
render(128).save("src-tauri/icons/128x128.png")
render(256).save("src-tauri/icons/128x128@2x.png")

# icns：用系统 iconutil
r = subprocess.run(["iconutil", "-c", "icns", ICONSET, "-o", "src-tauri/icons/icon.icns"],
                   capture_output=True, text=True)
print("icns:", r.returncode, r.stderr.strip())

# ico：Pillow 直接写多尺寸
imgs = [render(s) for s in (16, 24, 32, 48, 64, 128, 256)]
imgs[0].save("src-tauri/icons/icon.ico", format="ICO",
             sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
print("ico done")
