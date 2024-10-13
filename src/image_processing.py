from PIL import Image
import os
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

def convert_to(image_format: str, image_paths, compression_level=None, output_path=None, log_func=None):
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

                save_kwargs = get_compression_kwargs(image_format, compression_level)

                save_image(image, image_output_path, image_format, save_kwargs)

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

def get_compression_kwargs(image_format, compression_level):
    save_kwargs = {}
    if compression_level is None:
        return save_kwargs
    if image_format in ['jpeg', 'webp']:
        if compression_level == 'low':
            save_kwargs['quality'] = 90
        elif compression_level == 'medium':
            save_kwargs['quality'] = 70
        elif compression_level == 'high':
            save_kwargs['quality'] = 50
    elif image_format == 'png':
        if compression_level == 'low':
            save_kwargs['compress_level'] = 1
        elif compression_level == 'medium':
            save_kwargs['compress_level'] = 6
        elif compression_level == 'high':
            save_kwargs['compress_level'] = 9
    return save_kwargs

def save_image(image, output_path, image_format, save_kwargs):
    if image_format == 'png' and shutil.which('pngquant'):
        temp_path = output_path + '.png'
        image.save(temp_path, format='PNG')
        compression_level = save_kwargs.get('compress_level', 6)
        quality_map = {1: "80-100", 6: "60-80", 9: "40-60"}
        quality = quality_map.get(compression_level, "60-80")
        subprocess.run(['pngquant', '--quality', quality, '--output', output_path, temp_path], check=True)
        os.remove(temp_path)
    else:
        if image_format == 'png':
            image.save(output_path, image_format.upper(), optimize=True, **save_kwargs)
        else:
            if image_format == 'jpeg' and image.mode in ('RGBA', 'LA'):
                image = image.convert('RGB')
            image.save(output_path, image_format.upper(), **save_kwargs)

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
