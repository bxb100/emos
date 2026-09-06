"""Generate local cover images with the styles from emby-toolkit."""

import argparse
import math
import shutil
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image, ImageFont

from styles.style_multi_1 import create_style_multi_1
from styles.style_single_1 import create_style_single_1
from styles.style_single_2 import create_style_single_2

SCRIPT_DIR = Path(__file__).resolve().parent
COVERS_DIR = SCRIPT_DIR.parents[1] / "data" / "covers"
STYLES = {
    "single_1": create_style_single_1,
    "single_2": create_style_single_2,
    "multi_1": create_style_multi_1,
}


def collect_images(paths: list[Path]) -> list[Path]:
    """Expand directories in numeric filename order, preserving explicit file order."""
    images = []
    for path in paths:
        if path.is_dir():
            images.extend(sorted(
                (p for p in path.iterdir()
                 if p.is_file() and p.suffix.lower() in {".jpg", ".jpeg", ".png", ".webp"}),
                key=lambda p: (0, int(p.stem), p.name) if p.stem.isdecimal()
                else (1, p.name.casefold(), p.name),
            ))
        elif path.is_file():
            images.append(path)
        else:
            raise FileNotFoundError(f"图片路径不存在：{path}")
    if not images:
        raise ValueError("未找到图片，请提供图片文件或包含图片的目录")
    return images


def generate_cover(
    image_paths: list[Path],
    title: tuple[str, str],
    output_path: Path,
    *,
    style: str = "multi_1",
    font_path: tuple[Path, Path] | None = None,
    font_size: tuple[float, float] = (1.0, 1.0),
    blur: bool = False,
    blur_size: float = 50,
    color_ratio: float = 0.8,
    item_count: int | None = None,
    badge_style: str = "badge",
    badge_size_ratio: float = 0.12,
) -> Path:
    """Write a 1920×1080 PNG, using up to nine images and cycling if fewer are given.

    Single-image styles use the first image. Input images are never modified.
    Invalid inputs raise ValueError or OSError before the output is written.
    """
    if style not in STYLES:
        raise ValueError(f"未知风格：{style}")
    if not image_paths:
        raise ValueError("至少需要一张图片")
    if any(not math.isfinite(size) or size <= 0 for size in font_size):
        raise ValueError("字体缩放必须是大于 0 的有限数值")
    if not math.isfinite(blur_size) or blur_size < 0:
        raise ValueError("模糊半径必须是大于等于 0 的有限数值")
    if not 0 <= color_ratio <= 1:
        raise ValueError("背景混色比例必须在 0 到 1 之间")
    if item_count is not None and item_count < 0:
        raise ValueError("媒体数量不能为负数")
    if badge_style not in {"badge", "ribbon"} or not 0 < badge_size_ratio <= 1:
        raise ValueError("角标样式必须为 badge 或 ribbon，尺寸比例必须大于 0 且不超过 1")

    if font_path is None:
        font_path = (
            SCRIPT_DIR / "fonts" / ("zh_font_multi_1.ttf" if style == "multi_1" else "zh_font.ttf"),
            SCRIPT_DIR / "fonts" / ("en_font_multi_1.otf" if style == "multi_1" else "en_font.ttf"),
        )
    for font in font_path:
        if not font.is_file():
            raise FileNotFoundError(f"字体文件不存在：{font}")
        ImageFont.truetype(str(font), 24)

    selected = image_paths[:9] if style == "multi_1" else image_paths[:1]
    for path in selected:
        with Image.open(path) as image:
            image.load()

    options = {
        "font_size": font_size,
        "blur_size": blur_size,
        "color_ratio": color_ratio,
        "item_count": item_count,
        "config": {
            "show_item_count": item_count is not None,
            "badge_style": badge_style,
            "badge_size_ratio": badge_size_ratio,
        },
    }
    fonts = tuple(str(font) for font in font_path)
    if style == "multi_1":
        # Upstream expects 1.jpg through 9.jpg; staging also prevents stale posters on reruns.
        with TemporaryDirectory(prefix="cover-generator-") as directory:
            for index in range(9):
                shutil.copyfile(selected[index % len(selected)], Path(directory) / f"{index + 1}.jpg")
            image_data = STYLES[style](directory, title, fonts, is_blur=blur, **options)
    else:
        image_data = STYLES[style](str(selected[0]), title, fonts, **options)
    if not image_data:
        raise ValueError("封面生成失败，请检查图片和字体")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(image_data)
    return output_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="从本地图片生成媒体库封面（PNG，1920×1080）")
    parser.add_argument("namespace", help="默认输入目录和输出文件名，如 scifi")
    parser.add_argument("zh", help="中文标题")
    parser.add_argument("en", help="英文标题，可传空字符串")
    parser.add_argument("images", type=Path, nargs="*", help="图片或目录；默认 data/covers/<namespace>")
    parser.add_argument("--style", choices=STYLES, default="multi_1", help="封面风格（默认 multi_1）")
    parser.add_argument("--output", type=Path, help="输出 PNG 路径；默认 data/covers/<namespace>.png")
    parser.add_argument("--zh-font", type=Path, help="自定义中文字体")
    parser.add_argument("--en-font", type=Path, help="自定义英文字体")
    parser.add_argument("--font-size", type=float, nargs=2, default=(1.0, 1.0), metavar=("ZH", "EN"), help="中英文字体缩放")
    parser.add_argument("--blur", action="store_true", help="多图风格使用模糊背景")
    parser.add_argument("--blur-size", type=float, default=50, help="模糊半径（默认 50）")
    parser.add_argument("--color-ratio", type=float, default=0.8, help="背景混色比例，0–1（默认 0.8）")
    parser.add_argument("--item-count", type=int, help="显示媒体数量角标")
    parser.add_argument("--badge-style", choices=("badge", "ribbon"), default="badge")
    parser.add_argument("--badge-size-ratio", type=float, default=0.12, help="角标尺寸比例（默认 0.12）")
    args = parser.parse_intermixed_args(argv)
    if not args.namespace or args.namespace in {".", ".."} or "/" in args.namespace or "\\" in args.namespace:
        parser.error("namespace 必须是单个目录名")
    fonts_dir = SCRIPT_DIR / "fonts"
    fonts = (
        args.zh_font or fonts_dir / ("zh_font_multi_1.ttf" if args.style == "multi_1" else "zh_font.ttf"),
        args.en_font or fonts_dir / ("en_font_multi_1.otf" if args.style == "multi_1" else "en_font.ttf"),
    )
    try:
        output = generate_cover(
            collect_images(args.images or [COVERS_DIR / args.namespace]),
            (args.zh, args.en),
            args.output or COVERS_DIR / f"{args.namespace}.png",
            style=args.style,
            font_path=fonts,
            font_size=tuple(args.font_size),
            blur=args.blur,
            blur_size=args.blur_size,
            color_ratio=args.color_ratio,
            item_count=args.item_count,
            badge_style=args.badge_style,
            badge_size_ratio=args.badge_size_ratio,
        )
    except (OSError, ValueError) as error:
        print(f"生成失败：{error}", file=sys.stderr)
        return 1
    print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
