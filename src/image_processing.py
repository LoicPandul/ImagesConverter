from PIL import Image
import os
import io
import subprocess
import shutil

def is_same_format(target_format, image_path):
    _, ext = os.path.splitext(image_path)
    return ext.lower() == f'.{target_format.lower()}' or (ext.lower() == '.jpg' and target_format.lower() == 'jpeg')

def has_transparency_image(image):
    if image.mode in ('RGBA', 'LA'):
        return True
    elif image.mode == 'P':
        if 'transparency' in image.info:
            return True
    return False

def convert_to(image_format: str, image_paths, max_size_kb=None, output_path=None, log_func=None):
    image_format = image_format.lower()
    if image_format not in ["jpeg", "png", "webp"]:
        if log_func:
            log_func(f"{image_format} is not a valid format!")
        return False

    success = True
    for image_path in image_paths:
        file_name = os.path.basename(image_path)
        try:
            if not os.path.isfile(image_path):
                if log_func:
                    log_func(f"{image_path} is not a valid file!")
                continue
            with Image.open(image_path) as image:
                if image_format == 'jpeg' and has_transparency_image(image):
                    if log_func:
                        log_func(f"Failed to convert {file_name} to JPEG: image contains transparency.")
                    success = False
                    continue

                if output_path:
                    image_output_path = output_path
                else:
                    image_output_path = os.path.splitext(image_path)[0] + '.' + image_format

                if max_size_kb is not None:
                    compressed = compress_to_target_size(image, image_format, max_size_kb, log_func, file_name)
                    if compressed is not None:
                        with open(image_output_path, 'wb') as f:
                            f.write(compressed)
                    else:
                        save_image(image, image_output_path, image_format)
                else:
                    save_image(image, image_output_path, image_format)

                if image_path == image_output_path:
                    if log_func:
                        log_func(f"  - {file_name} processed and saved.")
                else:
                    if log_func:
                        log_func(f"  - {file_name} converted to {os.path.basename(image_output_path)}.")
        except Exception as e:
            if log_func:
                log_func(f"Failed to convert {file_name} to {image_format}. Error: {e}")
            success = False
            if output_path and os.path.exists(output_path):
                os.remove(output_path)
    return success

def compress_to_target_size(image, image_format, max_size_kb, log_func=None, file_name=None):
    target_bytes = max_size_kb * 1024

    if image_format == 'png':
        return compress_png_to_target(image, target_bytes, log_func, file_name)
    else:
        return compress_lossy_to_target(image, image_format, target_bytes, log_func, file_name)

def compress_lossy_to_target(image, image_format, target_bytes, log_func=None, file_name=None):
    img = image.copy()
    if image_format == 'jpeg' and img.mode in ('RGBA', 'LA'):
        img = img.convert('RGB')

    # First check: is the image already under target at max quality?
    buf = io.BytesIO()
    img.save(buf, format=image_format.upper(), quality=95)
    if buf.tell() <= target_bytes:
        return buf.getvalue()

    # Binary search on quality
    low, high = 1, 95
    best_data = None

    while low <= high:
        mid = (low + high) // 2
        buf = io.BytesIO()
        img.save(buf, format=image_format.upper(), quality=mid)
        size = buf.tell()

        if size <= target_bytes:
            best_data = buf.getvalue()
            low = mid + 1
        else:
            high = mid - 1

    if best_data is None:
        # Even quality=1 is too large, try resizing
        best_data = compress_with_resize(img, image_format, target_bytes, log_func, file_name)

    return best_data

def compress_with_resize(image, image_format, target_bytes, log_func=None, file_name=None):
    img = image.copy()
    for scale in [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1]:
        new_w = max(1, int(img.width * scale))
        new_h = max(1, int(img.height * scale))
        resized = image.resize((new_w, new_h), Image.LANCZOS)
        if image_format == 'jpeg' and resized.mode in ('RGBA', 'LA'):
            resized = resized.convert('RGB')

        # Try quality binary search on resized image
        low, high = 1, 95
        best_data = None
        while low <= high:
            mid = (low + high) // 2
            buf = io.BytesIO()
            resized.save(buf, format=image_format.upper(), quality=mid)
            size = buf.tell()
            if size <= target_bytes:
                best_data = buf.getvalue()
                low = mid + 1
            else:
                high = mid - 1

        if best_data is not None:
            if log_func and file_name:
                log_func(f"  - {file_name} resized to {new_w}x{new_h} to meet target size.")
            return best_data

    if log_func and file_name:
        log_func(f"  - Warning: Could not compress {file_name} to target size.")
    # Return smallest possible as fallback
    buf = io.BytesIO()
    tiny = image.resize((max(1, image.width // 10), max(1, image.height // 10)), Image.LANCZOS)
    if image_format == 'jpeg' and tiny.mode in ('RGBA', 'LA'):
        tiny = tiny.convert('RGB')
    tiny.save(buf, format=image_format.upper(), quality=1)
    return buf.getvalue()

def compress_png_to_target(image, target_bytes, log_func=None, file_name=None):
    # PNG is lossless, so we first try max compression
    buf = io.BytesIO()
    image.save(buf, format='PNG', optimize=True, compress_level=9)
    if buf.tell() <= target_bytes:
        return buf.getvalue()

    # If pngquant is available, try lossy PNG compression
    if shutil.which('pngquant'):
        for min_q, max_q in [(60, 80), (40, 60), (20, 40), (0, 20)]:
            buf = io.BytesIO()
            image.save(buf, format='PNG')
            png_data = buf.getvalue()

            import tempfile
            with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as tmp_in:
                tmp_in.write(png_data)
                tmp_in_path = tmp_in.name
            tmp_out_path = tmp_in_path + '_out.png'
            try:
                subprocess.run(
                    ['pngquant', '--quality', f'{min_q}-{max_q}', '--output', tmp_out_path, '--force', tmp_in_path],
                    check=True, capture_output=True
                )
                if os.path.getsize(tmp_out_path) <= target_bytes:
                    with open(tmp_out_path, 'rb') as f:
                        result = f.read()
                    return result
            except subprocess.CalledProcessError:
                pass
            finally:
                if os.path.exists(tmp_in_path):
                    os.remove(tmp_in_path)
                if os.path.exists(tmp_out_path):
                    os.remove(tmp_out_path)

    # Fallback: convert to RGBA/RGB palette reduction + resize
    for scale in [1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1]:
        new_w = max(1, int(image.width * scale))
        new_h = max(1, int(image.height * scale))
        resized = image.resize((new_w, new_h), Image.LANCZOS)

        # Try quantizing colors
        for colors in [256, 128, 64, 32, 16]:
            try:
                quantized = resized.quantize(colors=colors)
                buf = io.BytesIO()
                quantized.save(buf, format='PNG', optimize=True, compress_level=9)
                if buf.tell() <= target_bytes:
                    if scale < 1.0 and log_func and file_name:
                        log_func(f"  - {file_name} resized to {new_w}x{new_h} to meet target size.")
                    return buf.getvalue()
            except Exception:
                continue

    if log_func and file_name:
        log_func(f"  - Warning: Could not compress {file_name} to target size.")
    buf = io.BytesIO()
    image.save(buf, format='PNG', optimize=True, compress_level=9)
    return buf.getvalue()

def save_image(image, output_path, image_format):
    if image_format == 'jpeg' and image.mode in ('RGBA', 'LA'):
        image = image.convert('RGB')
    image.save(output_path, image_format.upper())

def clean_metadata(image_paths, log_func=None):
    for image_path in image_paths:
        try:
            img = Image.open(image_path)
            data = list(img.getdata())
            img_without_metadata = Image.new(img.mode, img.size)
            img_without_metadata.putdata(data)
            img_without_metadata.save(image_path)
        except Exception as e:
            if log_func:
                log_func(f"An error occurred while cleaning metadata of {image_path}: {e}")
