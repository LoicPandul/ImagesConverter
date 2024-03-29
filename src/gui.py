from PySide6.QtWidgets import QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit, \
    QGraphicsView, QGraphicsScene, QGraphicsPixmapItem, QGraphicsTextItem
from PySide6.QtCore import Qt
from PySide6.QtGui import QDragEnterEvent, QDropEvent, QPixmap, QIcon
import sys
import os

from .image_processing import convert_to, clean_metadata


class DragDropLabel(QGraphicsView):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.imageConverterGUI = parent
        
        scene = QGraphicsScene(0, 0, 1000, 450)
        self.setScene(scene)
        
        pixmap = QPixmap('assets/image_icon.png').scaled(100, 100, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        pixmapItem = QGraphicsPixmapItem(pixmap)
        pixmapItem.setPos((scene.width() - pixmap.width()) / 2, (scene.height() - pixmap.height()) / 2 - 20)
        scene.addItem(pixmapItem)
        
        textItem = QGraphicsTextItem("Drag and drop images here!")
        textItem.setPos((scene.width() - textItem.boundingRect().width()) / 2, (scene.height() / 2) + 60)
        scene.addItem(textItem)
        
        self.setAcceptDrops(True)
        self.setStyleSheet("border: 1px dashed #ccc;")
        
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
            print(f"Dropped file: {file}")
            if self.imageConverterGUI.conversion_mode:
                self.imageConverterGUI.convert_image(file)
        event.acceptProposedAction()


class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Image Converter")
        self.setFixedSize(1000, 1000)
        
        icon = QIcon('./assets/icon.ico')
        
        self.setWindowIcon(icon)
        
        self.conversion_mode = None
        
        layout = QVBoxLayout()
        
        self.drop_label = DragDropLabel(self)
        layout.addWidget(self.drop_label)
        
        self.btn_to_jpeg = QPushButton("Convert to JPEG")
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('jpeg'))
        layout.addWidget(self.btn_to_jpeg)
        
        self.btn_to_webp = QPushButton("Convert to WEBP")
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('webp'))
        layout.addWidget(self.btn_to_webp)
        
        self.btn_to_png = QPushButton("Convert to PNG")
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('png'))
        layout.addWidget(self.btn_to_png)
        
        self.message_terminal = QTextEdit()
        self.message_terminal.setReadOnly(True)
        layout.addWidget(self.message_terminal)
        
        central_widget = QWidget()
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)
        
        self.append_message("Select a conversion target by clicking a button to start.")
    
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
        self.append_message(f"Mode : to {mode}")
    
    def reset_button_styles(self):
        self.btn_to_jpeg.setStyleSheet("")
        self.btn_to_webp.setStyleSheet("")
        self.btn_to_png.setStyleSheet("")
    
    def convert_image(self, image_path):
        file_name = os.path.basename(image_path)
        try:
            clean_metadata([image_path])
            
            convert_to(self.conversion_mode, [image_path], self)

            self.append_message(f"  - {file_name} has been converted to {self.conversion_mode.upper()}.")
        except Exception as e:
            self.append_message(f"An error occurred while converting {file_name}: {e}")


if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec())
