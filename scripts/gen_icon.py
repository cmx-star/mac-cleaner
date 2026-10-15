#!/usr/bin/env python3
"""从 icon.svg 思路用 Pillow 直接栅格化 Mole 应用图标（无 cairo 依赖）。"""
from PIL import Image, ImageDraw, ImageFont
import math, sys

BLUE = (91, 163, 245)
PURPLE = (124, 91, 245)
DARK_BG1 = (32, 34, 41)
DARK_BG2 = (16, 17, 21)
BORDER = (58, 63, 77)
TUNNEL = (44, 47, 56)
NOSE = (229, 192, 123)
PUPIL = (22, 23, 27)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def render(size: int) -> Image.Image:
    S = 4
    px = size * S
    sc = px / 1024.0
    img = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # 圆角底板（macOS superellipse 近似）
    radius = int(220 * sc)
    mask = Image.new("L", (px, px), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, px - 1, px - 1], radius=radius, fill=255)

    # 对角渐变背景：逐行近似
    bg = Image.new("RGBA", (px, px))
    bd = ImageDraw.Draw(bg)
    for y in range(px):
        t = (y / px) * 0.85 + 0.05
        bd.line([(0, y), (px, y)], fill=lerp(DARK_BG1, DARK_BG2, t) + (255,))
    img.paste(bg, (0, 0), mask)

    # 地道弧线（二次贝塞尔）
    arc = []
    for i in range(101):
        t = i / 100
        x = (1 - t) ** 2 * 224 + 2 * (1 - t) * t * 512 + t ** 2 * 800
        y = (1 - t) ** 2 * 700 + 2 * (1 - t) * t * 470 + t ** 2 * 700
        arc.append((x * sc, y * sc))
    d.line(arc, fill=TUNNEL + (255,), width=int(28 * sc), joint="curve")
    for p in (arc[0], arc[-1]):
        r = 14 * sc
        d.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=TUNNEL + (255,))

    def vgrad_ellipse(cx, cy, rx, ry, top, bottom):
        """垂直渐变椭圆：逐行绘制。"""
        for i in range(int(ry * 2)):
            yy = cy - ry + i
            t = i / (ry * 2)
            col = lerp(top, bottom, t)
            half_w = rx * math.sqrt(max(0.0, 1 - ((yy - cy) / ry) ** 2))
            d.line([(cx - half_w) * sc, yy * sc, (cx + half_w) * sc, yy * sc],
                   fill=col + (255,))

    # 鼹鼠身体（上蓝下紫）
    vgrad_ellipse(512, 470, 150, 170, BLUE, PURPLE)
    # 耳朵
    for ex in (404, 620):
        d.ellipse([(ex - 44) * sc, (330 - 44) * sc, (ex + 44) * sc, (330 + 44) * sc],
                  fill=PURPLE + (255,))
        d.ellipse([(ex - 22) * sc, (330 - 22) * sc, (ex + 22) * sc, (330 + 22) * sc],
                  fill=(22, 23, 27, 255))
    # 眼睛
    for ex in (456, 568):
        d.ellipse([(ex - 26) * sc, (440 - 26) * sc, (ex + 26) * sc, (440 + 26) * sc],
                  fill=(255, 255, 255, 255))
    for ex, px_ in ((462, 462), (562, 562)):
        d.ellipse([(px_ - 13) * sc, (444 - 13) * sc, (px_ + 13) * sc, (444 + 13) * sc],
                  fill=PUPIL + (255,))
    # 鼻头
    d.ellipse([(512 - 40) * sc, (520 - 30) * sc, (512 + 40) * sc, (520 + 30) * sc],
              fill=NOSE + (255,))
    # 爪子
    for cx in (392, 632):
        d.ellipse([(cx - 48) * sc, (590 - 34) * sc, (cx + 48) * sc, (590 + 34) * sc],
                  fill=PURPLE + (255,))

    # 右下 M 徽章
    bx, by, br = 740, 760, 110
    d.ellipse([(bx - br) * sc, (by - br) * sc, (bx + br) * sc, (by + br) * sc],
              fill=(22, 23, 27, 255), outline=BORDER + (255,), width=int(6 * sc))
    # M 用折线绘制（不依赖字体）
    mw, mh = 92, 96  # M 尺寸
    mx, my = bx - mw / 2, by - mh / 2 + 6
    pts = [(mx, my + mh), (mx, my), (mx + mw / 2, my + mh * 0.55),
           (mx + mw, my), (mx + mw, my + mh)]
    lw = int(22 * sc)
    for i in range(len(pts) - 1):
        d.line([pts[i][0] * sc, pts[i][1] * sc, pts[i + 1][0] * sc, pts[i + 1][1] * sc],
               fill=BLUE + (255,), width=lw)
    d.ellipse([(mx - lw / 2) * sc, (my + mh - lw / 2) * sc, (mx + lw / 2) * sc, (my + mh + lw / 2) * sc],
              fill=BLUE + (255,))
    d.ellipse([(mx + mw - lw / 2) * sc, (my + mh - lw / 2) * sc, (mx + mw + lw / 2) * sc, (my + mh + lw / 2) * sc],
              fill=PURPLE + (255,))

    # 描边
    d.rounded_rectangle([2 * sc, 2 * sc, px - 2 * sc - 1, px - 2 * sc - 1],
                        radius=radius, outline=BORDER + (255,), width=int(4 * sc))

    return img.resize((size, size), Image.LANCZOS)


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/mole_icon_512.png"
    render(512).save(out)
    print("saved", out)
