import io
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from PIL import Image


PROJECT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_DIR))
import main as cover_main


class CoverGeneratorTests(unittest.TestCase):
    def setUp(self):
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.sources = self.root / "sources"
        self.sources.mkdir()
        self.output = self.root / "output.png"

    def make_image(self, name="poster.png", color=(180, 70, 40)):
        path = self.sources / name
        with Image.new("RGB", (120, 180), color) as image:
            image.save(path)
        return path

    @staticmethod
    def png_bytes(color=(180, 70, 40)):
        buffer = io.BytesIO()
        with Image.new("RGB", (8, 8), color) as image:
            image.save(buffer, format="PNG")
        return buffer.getvalue()

    def assert_cover(self, path):
        with Image.open(path) as image:
            self.assertEqual(image.format, "PNG")
            self.assertEqual(image.size, (1920, 1080))
            image.load()

    def test_multi_image_staging_preserves_sources_with_few_or_many_images(self):
        for count in (2, 12):
            with self.subTest(count=count):
                colors = [(20 + index * 13, 210 - index * 9, 30 + index * 15)
                          for index in range(count)]
                sources = [self.make_image(f"poster-{index}.png", color)
                           for index, color in enumerate(colors)]
                before = {path.name: path.read_bytes() for path in self.sources.iterdir()}
                staged_directories = []

                def render(directory, *args, **kwargs):
                    directory = Path(directory)
                    staged_directories.append(directory)
                    self.assertNotEqual(directory, self.sources)
                    posters = sorted(directory.glob("*.jpg"), key=lambda path: int(path.stem))
                    self.assertEqual(len(posters), 9)
                    staged_colors = []
                    for poster in posters:
                        with Image.open(poster) as image:
                            staged_colors.append(image.convert("RGB").getpixel((0, 0)))
                    self.assertEqual(set(staged_colors), set(colors[:9]))
                    return self.png_bytes()

                with patch.dict(cover_main.STYLES, {"multi_1": render}):
                    result = cover_main.generate_cover(sources, ("电影", "Movies"), self.output)

                self.assertEqual(result, self.output)
                self.assertEqual(self.output.read_bytes(), self.png_bytes())
                self.assertEqual(before, {path.name: path.read_bytes() for path in self.sources.iterdir()})
                self.assertTrue(staged_directories)
                self.assertTrue(all(not path.exists() for path in staged_directories))

    def test_regeneration_reads_updated_source_instead_of_cached_posters(self):
        source = self.make_image()
        observed_colors = []

        def render(directory, *args, **kwargs):
            colors = []
            for poster in Path(directory).glob("*.jpg"):
                with Image.open(poster) as image:
                    colors.append(image.convert("RGB").getpixel((0, 0)))
            self.assertEqual(len(colors), 9)
            self.assertEqual(len(set(colors)), 1)
            observed_colors.append(colors[0])
            return self.png_bytes(colors[0])

        with patch.dict(cover_main.STYLES, {"multi_1": render}):
            cover_main.generate_cover([source], ("电影", "Movies"), self.output)
            first_output = self.output.read_bytes()
            self.make_image(color=(40, 160, 90))
            cover_main.generate_cover([source], ("电影", "Movies"), self.output)

        self.assertEqual(observed_colors, [(180, 70, 40), (40, 160, 90)])
        self.assertNotEqual(first_output, self.output.read_bytes())
        self.assertEqual(list(self.sources.iterdir()), [source])

    def test_cli_directory_uses_numeric_order_and_preserves_title_spaces(self):
        for name in ("10.png", "2.png", "1.png"):
            self.make_image(name)
        (self.sources / "notes.txt").write_text("Not a poster", encoding="utf-8")
        captured = {}

        def generate(image_paths, title, output_path, **kwargs):
            captured.update(images=image_paths, title=title)
            return output_path

        with patch.object(cover_main, "generate_cover", side_effect=generate), redirect_stdout(io.StringIO()):
            result = cover_main.main([
                "movies", "电影 精选", "Movie Collection", str(self.sources),
                "--output", str(self.output),
            ])

        self.assertEqual(result, 0)
        self.assertEqual([path.name for path in captured["images"]], ["1.png", "2.png", "10.png"])
        self.assertEqual(captured["title"], ("电影 精选", "Movie Collection"))

    def test_invalid_cli_inputs_do_not_overwrite_an_existing_cover(self):
        source = self.make_image()
        empty_directory = self.root / "empty"
        empty_directory.mkdir()
        broken_image = self.sources / "broken.png"
        broken_image.write_bytes(b"This is not an image")
        truncated_image = self.make_image("truncated.jpg")
        truncated_image.write_bytes(truncated_image.read_bytes()[:-100])
        cases = (
            ("empty directory", [str(empty_directory)], "single_1"),
            ("corrupt image", [str(broken_image)], "single_1"),
            ("corrupt later poster", [str(source), str(truncated_image)], "multi_1"),
            ("missing image", [str(self.sources / "missing.png")], "single_1"),
            ("missing font", [str(source), "--zh-font", str(self.root / "missing.ttf")], "single_1"),
        )
        original_output = b"An existing cover must survive invalid input"
        for name, arguments, style in cases:
            with self.subTest(case=name):
                self.output.write_bytes(original_output)
                errors = io.StringIO()
                with redirect_stderr(errors), redirect_stdout(io.StringIO()):
                    result = cover_main.main([
                        "movies", "电影", "Movies", *arguments,
                        "--style", style, "--output", str(self.output),
                    ])
                self.assertNotEqual(result, 0)
                self.assertTrue(errors.getvalue().strip())
                self.assertNotIn("Traceback", errors.getvalue())
                self.assertEqual(self.output.read_bytes(), original_output)

    def test_renderer_failure_does_not_overwrite_an_existing_cover(self):
        source = self.make_image()

        def raise_io_error(*args, **kwargs):
            raise OSError("Unable to read poster during rendering")

        for renderer in (lambda *args, **kwargs: False, raise_io_error):
            with self.subTest(renderer=renderer.__name__):
                self.output.write_bytes(b"Existing cover")
                errors = io.StringIO()
                with patch.dict(cover_main.STYLES, {"single_1": renderer}), \
                        redirect_stderr(errors), redirect_stdout(io.StringIO()):
                    result = cover_main.main([
                        "movies", "电影", "Movies", str(source),
                        "--style", "single_1", "--output", str(self.output),
                    ])
                self.assertNotEqual(result, 0)
                self.assertTrue(errors.getvalue().strip())
                self.assertEqual(self.output.read_bytes(), b"Existing cover")

    def test_real_single_and_multi_renderers_produce_full_size_pngs(self):
        sources = [self.make_image(), self.make_image("second.png", (40, 160, 90))]
        for style, blur in (("single_2", False), ("multi_1", False), ("multi_1", True)):
            with self.subTest(style=style, blur=blur):
                output = self.root / f"{style}-{blur}.png"
                result = cover_main.generate_cover(
                    sources, ("电影精选", "Movie Collection"), output,
                    style=style, blur=blur, item_count=24,
                    badge_style="ribbon" if blur else "badge",
                )
                self.assertEqual(result, output)
                self.assert_cover(output)

    def test_cli_uses_bundled_fonts_from_an_unrelated_working_directory(self):
        source = self.make_image("poster with spaces.png")
        working_directory = self.root / "unrelated"
        working_directory.mkdir()
        result = subprocess.run(
            [sys.executable, str(PROJECT_DIR / "main.py"), "movies", "电影 精选",
             "Movie Collection", "--style", "single_1", str(source),
             "--output", "results/cover.png"],
            cwd=working_directory, capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_cover(working_directory / "results" / "cover.png")


if __name__ == "__main__":
    unittest.main()
