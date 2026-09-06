# Cover generator

从本地海报生成 1920×1080 PNG 封面，需要 Python 3.12+ 和 uv。

样式来源：[hbq0405/emby-toolkit](https://github.com/hbq0405/emby-toolkit/tree/4688d087443607828805b8d8fbdde0abc95f66aa/services/cover_generator)，
本次核对上游提交 `4688d087443607828805b8d8fbdde0abc95f66aa`（2026-09-05）。
同步多图圆角边界和 PNG 优化编码；本地样式直接返回 PNG bytes。
`main.py` 负责本地文件和命令行参数，字体使用随仓库提供的文件。

## 使用

在仓库根目录运行：

```sh
just gen scifi 科幻 'Science Fiction'
```

等价于在 `lib/cover_generator` 目录运行：

```sh
uv run main.py scifi 科幻 'Science Fiction'
```

默认读取仓库内 `data/covers/scifi/`，输出到 `data/covers/scifi.png`，使用多图风格 `multi_1`。
默认路径根据脚本位置解析，不受当前工作目录影响；原有每周生成工作流可继续使用。

在 `lib/cover_generator` 目录使用自定义图片、风格和输出位置：

```sh
# 单图：直接指定一张图片
uv run main.py movies 电影 Movies ./poster.jpg --style single_1 --output ./movies.png

# 多图：传入目录，也可依次传入多个图片文件或目录
uv run main.py shows 剧集 'TV Shows' ./posters --blur --item-count 128 --badge-style ribbon

# 自定义字体与缩放
uv run main.py movies 电影 Movies ./poster.jpg --style single_2 \
  --zh-font ./custom.ttf --font-size 1.1 0.9

uv run main.py --help
```

显式传入的相对路径根据当前工作目录解析。`namespace` 必须是单个目录名；自定义输出位置使用 `--output`。

- 可选风格：`single_1`、`single_2`、`multi_1`。
- 目录读取 JPG、JPEG、PNG、WebP 文件；纯数字文件名按数值排序，例如 `2.jpg` 排在 `10.jpg` 前。显式图片列表保持传入顺序。
- 单图使用第一张图片；多图使用前九张，少于九张时循环补齐，保留上游拼贴布局。编号适配在临时目录完成，源图不会被补写或覆盖，重复运行使用本次输入。
- 中文、英文字体默认随风格选择；`--zh-font`、`--en-font` 可分别替换。不需要下载字体或编辑配置字典。
- `--font-size ZH EN` 调整字体缩放；`--blur-size` 调整模糊半径，`--color-ratio` 调整背景混色比例。多图模糊背景通过 `--blur` 开启。
- `--item-count N` 显示媒体数量，支持 `--badge-style badge|ribbon` 和 `--badge-size-ratio`；沿用上游行为，数量为 0 时不绘制角标。
- 无图片、图片损坏、字体缺失或参数无效时返回非零状态；输入检查和渲染成功后才写入结果。

## 验证

```sh
cd lib/cover_generator
uv run --locked python -m unittest discover -s tests -v
# 或 just test
```

测试使用临时输入和输出，覆盖三种风格实际 PNG 渲染、命令行调用、图片补齐及错误处理。
