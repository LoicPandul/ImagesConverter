from PyQt5.QtWidgets import QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QLabel
from PyQt5.QtCore import Qt
from PyQt5.QtGui import QDragEnterEvent, QDropEvent
import sys

from .image_processing import convert_to_jpeg, convert_to_webp, convert_to_png

class DragDropLabel(QLabel):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.parent = parent 
        self.setText("Drag and drop files here")
        self.setAlignment(Qt.AlignCenter)
        self.setStyleSheet("border: 1px dashed #ccc; min-width: 1000px; min-height: 450px;") 
        self.setAcceptDrops(True)

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
            if self.parent.conversion_mode:
                self.parent.convert_image(file)
        event.acceptProposedAction()

class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Image Converter")
        self.setGeometry(100, 100, 400, 600) 

        self.conversion_mode = None

        layout = QVBoxLayout()

        self.btn_to_jpeg = QPushButton("Convert to JPEG")
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode('jpeg'))
        layout.addWidget(self.btn_to_jpeg)

        self.btn_to_webp = QPushButton("Convert to WEBP")
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode('webp'))
        layout.addWidget(self.btn_to_webp)

        self.btn_to_png = QPushButton("Convert to PNG")
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode('png'))
        layout.addWidget(self.btn_to_png)

        self.drop_label = DragDropLabel(self)
        layout.addWidget(self.drop_label)

        central_widget = QWidget()
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.reset_button_styles()
        if mode == 'jpeg':
            self.btn_to_jpeg.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'webp':
            self.btn_to_webp.setStyleSheet("background-color: #F2B807; color: black;")
        elif mode == 'png':
            self.btn_to_png.setStyleSheet("background-color: #F2B807; color: black;")
        print(f"Mode set to {mode}")

    def reset_button_styles(self):
        self.btn_to_jpeg.setStyleSheet("")
        self.btn_to_webp.setStyleSheet("")
        self.btn_to_png.setStyleSheet("")

    def convert_image(self, image_path):
        if self.conversion_mode == 'jpeg':
            convert_to_jpeg([image_path], None)
        elif self.conversion_mode == 'webp':
            convert_to_webp([image_path], None)
        elif self.conversion_mode == 'png':
            convert_to_png([image_path], None)

if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec_())

