from PIL import Image
import os


def is_same_format(target_format, image_path):
    _, ext = os.path.splitext(image_path)
    return ext.lower() == f'.{target_format.lower()}'


def convert_to_jpeg(image_paths, gui_instance):
    for image_path in image_paths:
        try:
            file_name = os.path.basename(image_path)
            if not os.path.isfile(image_path) or is_same_format('jpeg', image_path):
                continue
            
            with Image.open(image_path) as image:
                if image.mode in ("RGBA", "LA"):
                    image = image.convert("RGB")
                image_output_path = os.path.splitext(image_path)[0] + '.jpeg'
                image.save(image_output_path, 'JPEG')
            
            os.remove(image_path)
        except Exception as e:
            gui_instance.append_message(f"Échec de la conversion de {file_name} en JPEG. Erreur : {e}")


def convert_to_webp(image_paths, gui_instance):
    for image_path in image_paths:
        try:
            file_name = os.path.basename(image_path)
            if not os.path.isfile(image_path) or is_same_format('webp', image_path):
                continue
            
            with Image.open(image_path) as image:
                image_output_path = os.path.splitext(image_path)[0] + '.webp'
                image.save(image_output_path, 'WEBP')
            
            os.remove(image_path)
        except Exception as e:
            gui_instance.append_message(f"Échec de la conversion de {file_name} en WEBP. Erreur : {e}")


def convert_to_png(image_paths, gui_instance):
    for image_path in image_paths:
        try:
            file_name = os.path.basename(image_path)
            if not os.path.isfile(image_path) or is_same_format('png', image_path):
                continue
            
            with Image.open(image_path) as image:
                image_output_path = os.path.splitext(image_path)[0] + '.png'
                image.save(image_output_path, 'PNG')
            
            os.remove(image_path)
        except Exception as e:
            gui_instance.append_message(f"Échec de la conversion de {file_name} en PNG. Erreur : {e}")


def clean_metadata(image_paths):
    for image_path in image_paths:
        try:
            img = Image.open(image_path)
            data = list(img.getdata())
            img_without_metadata = Image.new(img.mode, img.size)
            img_without_metadata.putdata(data)
            img_without_metadata.save(image_path)
        except Exception as e:
            print(f"An error occurred while cleaning metadata of {image_path}: {str(e)}")
