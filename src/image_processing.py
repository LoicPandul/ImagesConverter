from PIL import Image
import os

def is_same_format(target_format, image_path):
    _, ext = os.path.splitext(image_path)
    return ext.lower() == f'.{target_format.lower()}'

def convert_to_jpeg(image_paths, gui_instance):
    for image_path in image_paths:
        if not os.path.isfile(image_path):
            print(f"Le fichier {image_path} n'existe pas.")
            continue
        
        if is_same_format('jpeg', image_path):
            print(f"Le fichier {image_path} est déjà au format JPEG, aucune conversion nécessaire.")
            continue

        with Image.open(image_path) as image:
            if image.mode in ("RGBA", "LA"):
                image = image.convert("RGB")
            image_output_path = os.path.splitext(image_path)[0] + '.jpeg'
            image.save(image_output_path, 'JPEG')
            print(f"{image_path} a été converti en {image_output_path}.")

        os.remove(image_path)
        print(f"{image_path} a été supprimé.")

def convert_to_webp(image_paths, gui_instance):
    for image_path in image_paths:
        if not os.path.isfile(image_path):
            print(f"Le fichier {image_path} n'existe pas.")
            continue
        
        if is_same_format('webp', image_path):
            print(f"Le fichier {image_path} est déjà au format WEBP, aucune conversion nécessaire.")
            continue

        with Image.open(image_path) as image:
            image_output_path = os.path.splitext(image_path)[0] + '.webp'
            image.save(image_output_path, 'WEBP')
            print(f"{image_path} a été converti en {image_output_path}.")

        os.remove(image_path)
        print(f"{image_path} a été supprimé.")

def convert_to_png(image_paths, gui_instance):
    for image_path in image_paths:
        if not os.path.isfile(image_path):
            print(f"Le fichier {image_path} n'existe pas.")
            continue

        if is_same_format('png', image_path):
            print(f"Le fichier {image_path} est déjà au format PNG, aucune conversion nécessaire.")
            continue

        with Image.open(image_path) as image:
            image_output_path = os.path.splitext(image_path)[0] + '.png'
            image.save(image_output_path, 'PNG')
            print(f"{image_path} a été converti en {image_output_path}.")

        os.remove(image_path)
        print(f"{image_path} a été supprimé.")

