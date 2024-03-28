from tkinter import Tk, Button, Label, PhotoImage, font
from tkinterdnd2 import DND_FILES, TkinterDnD
from PIL import Image, ImageTk
from .image_processing import convert_to_jpeg, convert_to_webp, convert_to_png
import re
import os

class ImageConverterGUI(TkinterDnD.Tk):
    def __init__(self):
        super().__init__()
        self.title("Image Converter")
        self.geometry("400x400")
        self.config(bg='#262624')

        self.custom_font = font.Font(family="Helvetica", size=12, weight="bold")
        
        default_bg_color = '#262624'
        default_fg_color = '#F2F2F2'
        self.color_to_jpeg = '#F25244'
        self.color_to_webp = '#F2E205'
        self.color_to_png = '#24F289'


        self.button_style = {'activebackground': default_bg_color, 'bg': default_bg_color, 'fg': default_fg_color, 'font': self.custom_font, 'highlightbackground': default_fg_color, 'highlightcolor': default_fg_color, 'highlightthickness': 1, 'borderwidth': 0, 'relief': 'solid'}

        self.active_button_jpeg_style = {'activebackground': self.color_to_jpeg, 'bg': self.color_to_jpeg, 'fg': 'white', 'font': self.custom_font}
        self.active_button_webp_style = {'activebackground': self.color_to_webp, 'bg': self.color_to_webp, 'fg': 'black', 'font': self.custom_font}
        self.active_button_png_style = {'activebackground': self.color_to_png, 'bg': self.color_to_png, 'fg': 'white', 'font': self.custom_font}


        self.btn_to_jpeg = Button(self, text="Convert to JPEG", command=lambda: self.set_conversion_mode('jpeg'), **self.button_style)
        self.btn_to_jpeg.pack(pady=10)

        self.btn_to_webp = Button(self, text="Convert to WEBP", command=lambda: self.set_conversion_mode('webp'), **self.button_style)
        self.btn_to_webp.pack(pady=10)

        self.btn_to_png = Button(self, text="Convert to PNG", command=lambda: self.set_conversion_mode('png'), **self.button_style)
        self.btn_to_png.pack(pady=10)

        original_icon_path = os.path.join(os.path.dirname(__file__), '..', 'assets', 'image_icon.png')
        original_icon = Image.open(original_icon_path)
        resized_icon = original_icon.resize((100, 100), Image.Resampling.LANCZOS)
        self.image_icon = ImageTk.PhotoImage(resized_icon)
        
        self.drop_label_text = "Drag and drop files here"
        self.drop_label = Label(self, text=self.drop_label_text, image=self.image_icon, compound='top', relief="groove", bg=default_bg_color, fg=default_fg_color, font=self.custom_font, highlightbackground=default_fg_color, highlightcolor=default_fg_color, highlightthickness=1)
        self.drop_label.pack(fill="both", expand=True, padx=10, pady=10)
        
        self.drop_label.drop_target_register(DND_FILES)
        self.drop_label.dnd_bind('<<Drop>>', self.handle_files_dropped)

        self.conversion_mode = None

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        text_mode = f"to {mode.upper()}"

        self.reset_button_colors()

        if mode == 'jpeg':
            self.btn_to_jpeg.config(**self.active_button_jpeg_style)
            self.drop_label.config(text=f"Drag and drop files here ({text_mode})", image=self.image_icon, compound='top', bg=self.color_to_jpeg, fg='white')
        elif mode == 'png':
            self.btn_to_png.config(**self.active_button_png_style)
            self.drop_label.config(text=f"Drag and drop files here ({text_mode})", image=self.image_icon, compound='top', bg=self.color_to_png, fg='white')
        else:  # 'webp'
            self.btn_to_webp.config(**self.active_button_webp_style)
            self.drop_label.config(text=f"Drag and drop files here ({text_mode})", image=self.image_icon, compound='top', bg=self.color_to_webp, fg='black')


    def reset_button_colors(self):
        self.btn_to_jpeg.config(**self.button_style)
        self.btn_to_webp.config(**self.button_style)
        self.btn_to_png.config(**self.button_style)

    def handle_files_dropped(self, event):
        if not self.conversion_mode:
            print("Please select a conversion mode.")
            return

        pattern = r'\{.*?\}'
        matches = re.findall(pattern, event.data)
        
        if matches:
            image_paths = [match.strip('{}') for match in matches]
        else:
            image_paths = event.data.strip().split()

        if self.conversion_mode == 'jpeg':
            convert_to_jpeg(image_paths, self)
        elif self.conversion_mode == 'webp':
            convert_to_webp(image_paths, self)
        elif self.conversion_mode == 'png':
            convert_to_png(image_paths, self)

