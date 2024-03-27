from PIL import Image
import os

def convert_to_jpeg(image_paths, gui_instance):
    for image_path in image_paths:
        if not os.path.isfile(image_path):
            print(f"The file {image_path} does not exist.")
            continue

        with Image.open(image_path) as image:
            if image.mode in ("RGBA", "LA"):
                image = image.convert("RGB")

            image_output_path = os.path.splitext(image_path)[0] + '.jpeg'
            image.save(image_output_path, 'JPEG')
            print(f"{image_path} has been converted to {image_output_path}.")

        os.remove(image_path)
        print(f"{image_path} has been deleted.")

def convert_to_webp(image_paths, gui_instance):
    for image_path in image_paths:
        if not os.path.isfile(image_path):
            print(f"The file {image_path} does not exist.")
            continue

        with Image.open(image_path) as image:
            image_output_path = os.path.splitext(image_path)[0] + '.webp'
            image.save(image_output_path, 'WEBP')
            print(f"{image_path} has been converted to {image_output_path}.")

        os.remove(image_path)
        print(f"{image_path} has been deleted.")
