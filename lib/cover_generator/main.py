import logging
import random
import shutil
import sys
from pathlib import Path
from typing import Dict, Any, List, Tuple, Optional

from styles.style_multi_1 import create_style_multi_1
from styles.style_single_1 import create_style_single_1
from styles.style_single_2 import create_style_single_2

UPDATING_IMAGES = set()

# Setup logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

workspace_root_dir = Path(__file__).resolve().parent.parent.parent


class CoverGeneratorService:
    SORT_BY_DISPLAY_NAME = {"Random": "随机", "Latest": "最新添加"}

    def __init__(self, config: Dict[str, Any]):
        self.config = config
        self._cover_style = self.config.get("cover_style", "single_1")
        self._multi_1_blur = self.config.get("multi_1_blur", False)
        self._multi_1_use_primary = self.config.get("multi_1_use_primary", True)
        self._single_use_primary = self.config.get("single_use_primary", False)
        self.data_path = Path(self.config.get("data_path", "./"))
        self.covers_path = workspace_root_dir / "data/covers"
        self.font_path = self.data_path / "fonts"
        self.covers_path.mkdir(parents=True, exist_ok=True)
        self.font_path.mkdir(parents=True, exist_ok=True)
        self.zh_font_path = None
        self.en_font_path = None
        self.zh_font_path_multi_1 = None
        self.en_font_path_multi_1 = None
        self._fonts_checked_and_ready = False

    def generate_cover(self, library_name: str, title: Tuple[str, str], image_paths: List[str],
                       item_count: Optional[int] = None) -> Optional[Path]:
        """
        Public method to generate a cover image.
        Returns the path to the generated image file.
        """
        self.__get_fonts()

        if not image_paths:
            logger.warning(f"  ➜ 为 '{library_name}' 没有提供图片路径")

        image_data = self.__generate_image_from_path(library_name, title, image_paths, item_count)

        save_path = Path(self.covers_path) / f"{library_name}.png"
        with open(save_path, "wb") as f:
            f.write(image_data)

        return save_path

    def __generate_image_from_path(self, library_name: str, title: Tuple[str, str], image_paths: List[str],
                                   item_count: Optional[int] = None) -> bytes:
        logger.debug(f"  ➜ 正在为 '{library_name}' 从本地路径生成封面...")
        zh_font_size = self.config.get("zh_font_size", 1)
        en_font_size = self.config.get("en_font_size", 1)
        blur_size = self.config.get("blur_size", 50)
        color_ratio = self.config.get("color_ratio", 0.8)
        font_size = (float(zh_font_size), float(en_font_size))

        if self._cover_style == 'single_1':
            return create_style_single_1(str(image_paths[0]), title, (str(self.zh_font_path), str(self.en_font_path)),
                                         font_size=font_size, blur_size=blur_size, color_ratio=color_ratio,
                                         item_count=item_count, config=self.config)
        elif self._cover_style == 'single_2':
            return create_style_single_2(str(image_paths[0]), title, (str(self.zh_font_path), str(self.en_font_path)),
                                         font_size=font_size, blur_size=blur_size, color_ratio=color_ratio,
                                         item_count=item_count, config=self.config)
        elif self._cover_style == 'multi_1':
            if self.zh_font_path_multi_1 and self.zh_font_path_multi_1.exists():
                zh_font_path_multi = self.zh_font_path_multi_1
            else:
                logger.warning(f"  ➜ 未找到多图专用中文字体 ({self.zh_font_path_multi_1})，将回退使用单图字体。")
                zh_font_path_multi = self.zh_font_path
            if self.en_font_path_multi_1 and self.en_font_path_multi_1.exists():
                en_font_path_multi = self.en_font_path_multi_1
            else:
                logger.warning(f"  ➜ 未找到多图专用英文字体 ({self.en_font_path_multi_1})，将回退使用单图字体。")
                en_font_path_multi = self.en_font_path
            font_path_multi = (str(zh_font_path_multi), str(en_font_path_multi))
            zh_font_size_multi = self.config.get("zh_font_size_multi_1", 1)
            en_font_size_multi = self.config.get("en_font_size_multi_1", 1)
            font_size_multi = (float(zh_font_size_multi), float(en_font_size_multi))
            blur_size_multi = self.config.get("blur_size_multi_1", 50)
            color_ratio_multi = self.config.get("color_ratio_multi_1", 0.8)
            library_dir = self.covers_path / library_name
            self.__prepare_multi_images(library_dir, image_paths)
            return create_style_multi_1(str(library_dir), title, font_path_multi, font_size=font_size_multi,
                                        is_blur=self._multi_1_blur, blur_size=blur_size_multi,
                                        color_ratio=color_ratio_multi, item_count=item_count, config=self.config)
        return None

    def __prepare_multi_images(self, library_dir: Path, source_paths: List[str]):
        library_dir.mkdir(parents=True, exist_ok=True)
        for i in range(1, 10):
            target_path = library_dir / f"{i}.jpg"
            if not target_path.exists():
                source_to_copy = random.choice(source_paths)
                shutil.copy(source_to_copy, target_path)

    def __get_fonts(self):
        if self._fonts_checked_and_ready:
            return
        font_definitions = [
            {"target_attr": "zh_font_path", "filename": "zh_font.ttf", "local_key": "zh_font_path_local",
             "url_key": "zh_font_url"},
            {"target_attr": "en_font_path", "filename": "en_font.ttf", "local_key": "en_font_path_local",
             "url_key": "en_font_url"}, {"target_attr": "zh_font_path_multi_1", "filename": "zh_font_multi_1.ttf",
                                         "local_key": "zh_font_path_multi_1_local", "url_key": "zh_font_url_multi_1"},
            {"target_attr": "en_font_path_multi_1", "filename": "en_font_multi_1.otf",
             "local_key": "en_font_path_multi_1_local", "url_key": "en_font_url_multi_1"}]
        for font_def in font_definitions:
            font_path_to_set = None
            expected_font_file = self.font_path / font_def["filename"]

            # Check for configured local path first
            local_path_str = self.config.get(font_def["local_key"])
            if local_path_str:
                local_path = Path(local_path_str)
                if local_path.exists():
                    logger.debug(f"  ➜ 发现并优先使用用户指定的外部字体: {local_path_str}")
                    font_path_to_set = local_path
                else:
                    logger.warning(f"  ➜ 配置的外部字体路径不存在: {local_path_str}，将忽略此配置。")

            # Then check if we already have it in our data path
            if not font_path_to_set and expected_font_file.exists():
                font_path_to_set = expected_font_file

            setattr(self, font_def["target_attr"], font_path_to_set)

        if self.zh_font_path and self.en_font_path:
            logger.debug("  ➜ 核心字体文件已准备就绪。后续任务将不再重复检查。")
            self._fonts_checked_and_ready = True
        else:
            logger.warning("  ➜ 一个或多个核心字体文件缺失且无法下载。请检查UI中的本地路径或下载链接是否有效。")


def main():
    # Use the sample image we copied
    script_dir = Path(__file__).resolve().parent

    # Configuration based on available fonts on this macOS machine
    config = {
        "data_path": Path(script_dir),
        "cover_style": "multi_1",  # 可选: "single_1", "single_2",
        "tab": "style-tab",  # 媒体数量开关
        "show_item_count": False,  # 默认为关闭
        # 单图风格设置
        "zh_font_path_local": "", "en_font_path_local": "", "zh_font_url": "", "en_font_url": "", "zh_font_size": 1,
        "en_font_size": 1, "blur_size": 50, "color_ratio": 0.8, "single_use_primary": False,  # 多图风格1设置
        "zh_font_path_multi_1_local": "", "en_font_path_multi_1_local": "", "zh_font_url_multi_1": "",
        "en_font_url_multi_1": "", "zh_font_size_multi_1": 1.0, "en_font_size_multi_1": 1.0, "blur_size_multi_1": 50,
        "color_ratio_multi_1": 0.8, "multi_1_blur": False, "multi_1_use_main_font": False,
        "multi_1_use_primary": True,
    }

    # Initialize service
    service = CoverGeneratorService(config)

    # Generate the cover
    library_name = sys.argv[1] if len(sys.argv) > 1 else "default"
    title = (sys.argv[2], sys.argv[3]) if len(sys.argv) > 3 else ("默认", "Default Media")

    logger.info(f"Generating cover for '{library_name}' using style '{config['cover_style']}'...")
    output_path = service.generate_cover(library_name, title, [])

    if output_path:
        logger.info(f"Cover generated successfully: {output_path}")
    else:
        logger.error("FAILED to generate cover. Check logs for details.")


if __name__ == "__main__":
    main()
