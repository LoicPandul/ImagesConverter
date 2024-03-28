from PyQt5.QtWidgets import QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit, QLabel
from PyQt5.QtCore import Qt
from PyQt5.QtGui import QDragEnterEvent, QDropEvent, QPixmap
import sys
import os

from .image_processing import convert_to_jpeg, convert_to_webp, convert_to_png

class DragDropLabel(QLabel):
    def __init__(self, imageConverterGUI=None):
        super().__init__(imageConverterGUI)
        self.imageConverterGUI = imageConverterGUI
        self.setAcceptDrops(True)
        self.setAlignment(Qt.AlignCenter)
        self.setStyleSheet("border: 1px dashed #ccc; min-width: 1000px; min-height: 450px;")
        self.setText("\n\nDrag and drop images here!")

        pixmap = QPixmap('assets/image_icon.png').scaled(100, 100, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        self.setPixmap(pixmap)

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
            print(f"Dropped file: {file}")
            if self.imageConverterGUI.conversion_mode:
                self.imageConverterGUI.convert_image(file)
        event.acceptProposedAction()

class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Image Converter")
        self.setGeometry(100, 100, 400, 600)

        self.conversion_mode = None

        layout = QVBoxLayout()

        self.drop_label = DragDropLabel(self)
        layout.addWidget(self.drop_label)

        self.btn_to_jpeg = QPushButton("Convert to JPEG")
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode('jpeg'))
        layout.addWidget(self.btn_to_jpeg)

        self.btn_to_webp = QPushButton("Convert to WEBP")
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode('webp'))
        layout.addWidget(self.btn_to_webp)

        self.btn_to_png = QPushButton("Convert to PNG")
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode('png'))
        layout.addWidget(self.btn_to_png)

        self.message_terminal = QTextEdit()
        self.message_terminal.setReadOnly(True)
        layout.addWidget(self.message_terminal)

        central_widget = QWidget()
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

    def append_message(self, message):
        self.message_terminal.append(message)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.reset_button_styles()
        if mode == 'jpeg':
            self.btn_to_jpeg.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'webp':
            self.btn_to_webp.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'png':
            self.btn_to_png.setStyleSheet("background-color: #F2B807; color: black;")
        self.append_message(f"Mode : to {mode}")

    def reset_button_styles(self):
        self.btn_to_jpeg.setStyleSheet("")
        self.btn_to_webp.setStyleSheet("")
        self.btn_to_png.setStyleSheet("")

    def convert_image(self, image_path):
        file_name = os.path.basename(image_path)
        try:
            if self.conversion_mode == 'jpeg':
                convert_to_jpeg([image_path], self)
            elif self.conversion_mode == 'webp':
                convert_to_webp([image_path], self)
            elif self.conversion_mode == 'png':
                convert_to_png([image_path], self)

            self.append_message(f"  - {file_name} has been converted to {self.conversion_mode.upper()}.")
        except Exception as e:
            self.append_message(f"An error occurred while converting {file_name}: {str(e)}")

if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec_())

