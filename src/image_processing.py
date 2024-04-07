from PIL import Image
import os

def process_image(image_path, format, signals):
    try:
        original_format = os.path.splitext(image_path)[-1].lower()
        target_format = f'.{format.lower()}'
        with Image.open(image_path) as img:
            file_name = os.path.basename(image_path)
            img, metadata_message = clean_metadata(img)
            if original_format != target_format: 
                img, conversion_message = convert_to(format, img, image_path)
                os.remove(image_path) 
                signals.finished.emit(f"{conversion_message}. {metadata_message}.")
            else:  
                img.save(image_path)
                signals.finished.emit(f"{file_name} format is already {format.upper()}. {metadata_message}.")
    except Exception as e:
        signals.error.emit(f"Error processing {file_name}: {str(e)}")

def clean_metadata(img):
    data = list(img.getdata())
    clean_img = Image.new(img.mode, img.size)
    clean_img.putdata(data)
    return clean_img, "Metadata cleaned"

def convert_to(format, img, image_path):
    if format not in ["jpeg", "png", "webp"]:
        return img, f"{format} is not a valid format!"
    
    output_path = os.path.splitext(image_path)[0] + '.' + format
    if format == 'webp':
        img.save(output_path, format.upper(), quality=90)
    else:
        if img.mode in ("RGBA", "LA") and format != 'png':
            img = img.convert("RGB")
        img.save(output_path, format.upper())
    
    return img, f"{os.path.basename(image_path)} has been converted to {format.upper()}"