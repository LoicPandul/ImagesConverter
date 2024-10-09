from PySide6.QtWidgets import QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit, \
    QGraphicsView, QGraphicsScene, QGraphicsPixmapItem, QGraphicsTextItem, QCheckBox, QLabel, QComboBox, QHBoxLayout
from PySide6.QtCore import Qt
from PySide6.QtGui import QDragEnterEvent, QDropEvent, QPixmap, QIcon
from PIL import Image
import sys
import os
import shutil

from .image_processing import convert_to, clean_metadata, is_same_format, has_transparency_image

if getattr(sys, 'frozen', False):
    application_path = sys._MEIPASS
else:
    application_path = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

icon_path = os.path.join(application_path, 'assets', 'icon.ico')
image_icon_path = os.path.join(application_path, 'assets', 'image_icon.png')

class DragDropLabel(QGraphicsView):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.parent = parent

        scene = QGraphicsScene(0, 0, 600, 450)
        self.setScene(scene)

        pixmap = QPixmap(image_icon_path).scaled(100, 100, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        pixmapItem = QGraphicsPixmapItem(pixmap)
        pixmapItem.setPos((scene.width() - pixmap.width()) / 2, (scene.height() - pixmap.height()) / 2 - 20)
        scene.addItem(pixmapItem)

        textItem = QGraphicsTextItem("Drag and drop images here!")
        textItem.setPos((scene.width() - textItem.boundingRect().width()) / 2, (scene.height() / 2) + 60)
        scene.addItem(textItem)

        self.setAcceptDrops(True)
        self.setStyleSheet("border: 1px dashed #ccc; background-color: #f0f0f0;")

        self.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)

    def dragEnterEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
        else:
            super().dragEnterEvent(event)

    def dragMoveEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
        else:
            super().dragMoveEvent(event)

    def dropEvent(self, event: QDropEvent):
        files = [url.toLocalFile() for url in event.mimeData().urls()]
        for file in files:
            print(f"File dropped: {file}")
            if self.parent.conversion_mode:
                self.parent.convert_image(file)
            else:
                self.parent.append_message("Please select a target format!")
        event.acceptProposedAction()

class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Image Converter")
        self.setFixedSize(600, 600)

        self.setWindowIcon(QIcon(icon_path))

        self.conversion_mode = None

        layout = QVBoxLayout()

        self.drop_label = DragDropLabel(self)
        layout.addWidget(self.drop_label)

        buttons_layout = QHBoxLayout()

        self.btn_to_jpeg = QPushButton("Convert to JPEG")
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('jpeg'))
        buttons_layout.addWidget(self.btn_to_jpeg)

        self.btn_to_webp = QPushButton("Convert to WEBP")
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('webp'))
        buttons_layout.addWidget(self.btn_to_webp)

        self.btn_to_png = QPushButton("Convert to PNG")
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('png'))
        buttons_layout.addWidget(self.btn_to_png)

        layout.addLayout(buttons_layout)

        self.checkbox_delete_original = QCheckBox("Delete original image after conversion")
        self.checkbox_delete_original.setChecked(True)
        layout.addWidget(self.checkbox_delete_original)

        compression_layout = QHBoxLayout()
        compression_label = QLabel("Compression Level:")
        compression_layout.addWidget(compression_label)

        self.compression_combo = QComboBox()
        self.compression_combo.addItem("No Compression")
        self.compression_combo.addItem("Low")
        self.compression_combo.addItem("Medium")
        self.compression_combo.addItem("High")
        self.compression_combo.setCurrentIndex(0)
        compression_layout.addWidget(self.compression_combo)

        layout.addLayout(compression_layout)

        self.message_terminal = QTextEdit()
        self.message_terminal.setReadOnly(True)
        layout.addWidget(self.message_terminal)

        central_widget = QWidget()
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

        self.append_message("Select a conversion format by clicking a button to start.")

    def append_message(self, message):
        self.message_terminal.append(message)

    def set_conversion_mode_and_clear_message(self, mode):
        self.message_terminal.clear()
        self.set_conversion_mode(mode)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.reset_button_styles()
        if mode == 'jpeg':
            self.btn_to_jpeg.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'webp':
            self.btn_to_webp.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'png':
            self.btn_to_png.setStyleSheet("background-color: #F2B807; color: black;")
        self.append_message(f"Mode: to {mode.upper()}")

    def reset_button_styles(self):
        self.btn_to_jpeg.setStyleSheet("")
        self.btn_to_webp.setStyleSheet("")
        self.btn_to_png.setStyleSheet("")

    def convert_image(self, image_path):
        file_name = os.path.basename(image_path)
        conversion_successful = False
        new_image_created = False
        original_handled = False
        try:
            if not self.is_format_supported(image_path):
                self.append_message(f"Format of {file_name} not supported.")
                return
            if self.conversion_mode == 'jpeg' and has_transparency(image_path):
                self.append_message(f"Conversion of {file_name} to JPEG is impossible: the image contains transparency.")
                return
            compression_level = self.get_compression_level()
            output_path = None

            if not self.checkbox_delete_original.isChecked():
                base, ext = os.path.splitext(image_path)
                working_image_path = f"{base}-temp{ext}"
                shutil.copy2(image_path, working_image_path)
            else:
                working_image_path = image_path

            clean_metadata([working_image_path], self)

            if is_same_format(self.conversion_mode, image_path):
                if compression_level is not None:
                    base, ext = os.path.splitext(image_path)
                    if self.checkbox_delete_original.isChecked():
                        output_path = f"{base}-temp_compress{ext}"
                    else:
                        output_path = f"{base}-compressed{ext}"
                    success = convert_to(self.conversion_mode, [working_image_path], self, compression_level, output_path)
                    if success:
                        self.append_message(f"{file_name} has been compressed.")
                        new_image_created = True
                        conversion_successful = True
                        if self.checkbox_delete_original.isChecked():
                            os.replace(output_path, image_path)
                            original_handled = True
                        else:
                            pass
                    else:
                        if output_path and os.path.exists(output_path):
                            os.remove(output_path)
                else:
                    if not self.checkbox_delete_original.isChecked():
                        base, ext = os.path.splitext(image_path)
                        output_path = f"{base}-clean{ext}"
                        shutil.move(working_image_path, output_path)
                        self.append_message(f"{file_name} metadata cleaned. New image created.")
                        new_image_created = True
                        conversion_successful = True
                    else:
                        os.replace(working_image_path, image_path)
                        self.append_message(f"{file_name} metadata cleaned.")
                        conversion_successful = True
                        new_image_created = False
            else:
                if not self.checkbox_delete_original.isChecked():
                    base, _ = os.path.splitext(image_path)
                    output_path = f"{base}-converted.{self.conversion_mode}"
                else:
                    output_path = None
                success = convert_to(self.conversion_mode, [working_image_path], self, compression_level, output_path)
                if success:
                    new_image_created = True
                    conversion_successful = True
                    if self.checkbox_delete_original.isChecked():
                        os.remove(image_path)
                        original_handled = True
                else:
                    conversion_successful = False
        except Exception as e:
            self.append_message(f"An error occurred while processing {file_name}: {e}")
            conversion_successful = False
        finally:
            if not self.checkbox_delete_original.isChecked():
                if working_image_path and os.path.exists(working_image_path):
                    os.remove(working_image_path)

        if self.checkbox_delete_original.isChecked() and conversion_successful and new_image_created and not original_handled:
            if os.path.exists(image_path):
                os.remove(image_path)
                original_handled = True

    def get_compression_level(self):
        level = self.compression_combo.currentIndex()
        if level == 0:
            return None
        elif level == 1:
            return 'low'
        elif level == 2:
            return 'medium'
        elif level == 3:
            return 'high'

    def is_format_supported(self, image_path):
        supported_formats = ['.jpeg', '.jpg', '.png', '.webp']
        _, ext = os.path.splitext(image_path)
        if ext.lower() in supported_formats:
            return True
        else:
            return False

def has_transparency(image_path):
    try:
        with Image.open(image_path) as img:
            if img.mode in ('RGBA', 'LA') or (img.mode == 'P' and 'transparency' in img.info):
                return True
            else:
                return False
    except Exception:
        return False

if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec())
