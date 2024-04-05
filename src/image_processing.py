from PIL import Image
import os

def is_same_format(target_format, image_path):
    _, ext = os.path.splitext(image_path)
    return ext.lower() == f'.{target_format.lower()}'

def convert_to(image_format: str, image_paths, gui_instance):
    image_format = image_format.lower()
    if image_format not in ["jpeg", "png", "webp"]:
        gui_instance.append_message(f"{image_format} is not a valid format!")
        return
    
    for image_path in image_paths:
        file_name = os.path.basename(image_path)
        try:
            if not os.path.isfile(image_path):
                gui_instance.append_message(f"{image_path} is not a valid file!")
                continue
            elif is_same_format(image_format, image_path):
                gui_instance.append_message(f"{image_path} is already in the format {image_format}")
                continue
            
            with Image.open(image_path) as image:
                image_output_path = os.path.splitext(image_path)[0] + '.' + image_format
                
                if image_format == 'webp':
                    if image.mode in ("RGBA", "LA"):
                        image.save(image_output_path, image_format.upper(), quality=90, lossless=True)
                    else:
                        image.save(image_output_path, image_format.upper(), quality=90)
                else:
                    if image.mode in ("RGBA", "LA") and image_format != 'png':
                        image = image.convert("RGB")
                    image.save(image_output_path, image_format.upper())
                
                gui_instance.append_message(f"  - {file_name} has been converted to {image_format.upper()}.")
                
                os.remove(image_path)
        except Exception as e:
            gui_instance.append_message(f"Failed to convert {file_name} to {image_format}. Error: {e}")

def clean_metadata(image_paths, gui_instance):
    for image_path in image_paths:
        try:
            img = Image.open(image_path)
            data = list(img.getdata())
            img_without_metadata = Image.new(img.mode, img.size)
            img_without_metadata.putdata(data)
            img_without_metadata.save(image_path)
        except Exception as e:
            gui_instance.append_message(f"An error occurred while cleaning metadata of {image_path}: {e}")
