from PySide6.QtWidgets import QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit, QGraphicsView, QGraphicsScene, QGraphicsPixmapItem, QGraphicsTextItem
from PySide6.QtCore import Qt, Signal, QObject
from PySide6.QtGui import QDragEnterEvent, QDropEvent, QPixmap, QIcon
import sys
import os
from concurrent.futures import ThreadPoolExecutor
from .image_processing import process_image

class WorkerSignals(QObject):
    finished = Signal(str)
    error = Signal(str)

class DragDropLabel(QGraphicsView):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.parent = parent
        scene = QGraphicsScene(0, 0, 600, 450)
        self.setScene(scene)
        self.setAcceptDrops(True)
        self.init_ui()
        
    def init_ui(self):
        pixmap = QPixmap(self.parent.image_icon_path).scaled(100, 100, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        pixmapItem = QGraphicsPixmapItem(pixmap)
        sceneWidth = self.scene().width()
        sceneHeight = self.scene().height()
        pixmapItem.setPos((sceneWidth - pixmap.width()) / 2, (sceneHeight - pixmap.height()) / 2 - 20)
        self.scene().addItem(pixmapItem)
        
        textItem = QGraphicsTextItem("Drag and drop images here!")
        textItem.setPos((sceneWidth - textItem.boundingRect().width()) / 2, (sceneHeight / 2) + 60)
        self.scene().addItem(textItem)
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
        self.parent.process_images(files)
        event.acceptProposedAction()

class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("ImagesConverter")
        self.setFixedSize(600, 450)
        self.executor = ThreadPoolExecutor(max_workers=4)
        self.conversion_mode = None
        self.image_icon_path = self.get_resource_path('image_icon.png')
        self.setup_ui()

    def get_resource_path(self, relative_path):
        base_path = os.path.abspath(os.path.dirname(__file__))
        return os.path.join(base_path, os.pardir, 'assets', relative_path)

    def setup_ui(self):
        self.setWindowIcon(QIcon(self.get_resource_path('icon.ico')))
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
        self.append_message("Select a conversion target by clicking a button to start.", add_dash=False)

    def append_message(self, message, add_dash=True):
        if add_dash:
            message = f"- {message}"
        self.message_terminal.append(message)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.message_terminal.clear()
        self.append_message(f"Conversion mode set to: {mode.upper()}", add_dash=False)
        self.reset_button_styles()
        button = getattr(self, f'btn_to_{mode}', None)
        if button:
            button.setStyleSheet("background-color: #F2B807; color: black;")
    
    def reset_button_styles(self):
        self.btn_to_jpeg.setStyleSheet("")
        self.btn_to_webp.setStyleSheet("")
        self.btn_to_png.setStyleSheet("")

    def process_images(self, image_paths):
        if not self.conversion_mode:
            self.append_message("Please select a conversion mode first.", add_dash=False)
            return
        self.message_terminal.clear()
        for image_path in image_paths:
            self.executor.submit(lambda p=image_path: self.convert_and_display_message(p))

    def convert_and_display_message(self, image_path):
        signals = WorkerSignals()
        signals.finished.connect(lambda msg: self.append_message(msg))
        signals.error.connect(lambda msg: self.append_message(f"Error: {msg}"))
        process_image(image_path, self.conversion_mode, signals)

if __name__ == "__main__":
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec_())

