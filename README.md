# ImagesConverter

ImagesConverter is a simple Python GUI application designed for converting images between different formats via drag-and-drop. It's not an incredible software, but it's a small tool I made for myself to increase productivity, and if it can be useful to you as well, all the better! I will try to improve the software over time.

## Features

Easily convert your images with a simple drag-and-drop into formats:
- JPEG;
- WEBP;
- PNG.

Superfluous metadata from your images is automatically removed during conversion.

## Prerequisites

Before launching the application, make sure you have Python installed on your system. Requires Python 3.10 or later.

For Windows:
```bash
python --version
```

For macOS and Linux:
```bash
python3 --version
```

## Installation

To use ImagesConverter, first clone this repository to your local machine using Git:

```bash
git clone https://github.com/LoicPandul/ImagesConverter.git
cd ImagesConverter
```

Then, install the necessary dependencies:

For Windows:
```bash
pip install -r requirements.txt
```

For macOS and Linux:
```bash
python3 -m pip install -r requirements.txt
```

If you experience any trouble here on macOS, try this command:
```bash
python3 -m pip install --upgrade pillow PySide6 PySide6_Addons PySide6_Essentials shiboken6
```

## How to Use

To start the software, navigate to the project folder in your terminal, then execute:

For Windows:
```bash
python run.py
```

For macOS and Linux:
```bash
python3 run.py
```

## License

CC0 1.0 Universal - Public Domain
https://creativecommons.org/publicdomain/zero/1.0/