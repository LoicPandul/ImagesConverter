from PySide6.QtWidgets import (
    QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit,
    QLabel, QCheckBox, QHBoxLayout, QFrame, QSizePolicy, QButtonGroup
)
from PySide6.QtCore import Qt, QUrl, QSize, QPropertyAnimation, QEasingCurve, QRect
from PySide6.QtGui import QDragEnterEvent, QDropEvent, QPixmap, QIcon, QFont
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

class DragDropWidget(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.parent = parent

        self.setAcceptDrops(True)
        self.setObjectName("DragDropWidget")
        self.layout = QVBoxLayout()
        self.layout.setContentsMargins(0, 0, 0, 0)  # Optionnel: Ajuster les marges si nécessaire
        self.setLayout(self.layout)

        self.icon_label = QLabel()
        pixmap = QPixmap(image_icon_path)
        if pixmap.isNull():
            print(f"Failed to load image {image_icon_path}")
        else:
            # Obtenir le ratio de pixels de l'écran
            device_ratio = self.devicePixelRatioF()

            # Redimensionner l'image en tenant compte du ratio de pixels (par exemple, 60x60 pixels)
            scaled_pixmap = pixmap.scaled(60 * device_ratio, 60 * device_ratio, Qt.KeepAspectRatio, Qt.SmoothTransformation)
            scaled_pixmap.setDevicePixelRatio(device_ratio)
            self.icon_label.setPixmap(scaled_pixmap)
            self.icon_label.setFixedSize(scaled_pixmap.size() / device_ratio)  # Ajuster la taille fixe en conséquence
        self.icon_label.setAlignment(Qt.AlignCenter)
        self.icon_label.setScaledContents(False)  # Assurer que l'image n'est pas redimensionnée automatiquement

        self.text_label = QLabel("DRAG AND DROP HERE")
        self.text_label.setAlignment(Qt.AlignCenter)
        self.text_label.setObjectName("DragDropText")
        font = self.text_label.font()
        font.setPointSize(font.pointSize() - 2)  # Réduire légèrement la taille de la police
        font.setBold(True)
        self.text_label.setFont(font)

        self.layout.addStretch()
        self.layout.addWidget(self.icon_label, alignment=Qt.AlignHCenter)
        self.layout.addSpacing(20)  # Ajoute un espace de 20 pixels entre l'image et le texte
        self.layout.addWidget(self.text_label, alignment=Qt.AlignHCenter)
        self.layout.addStretch()

        self.original_geometry = None  # Initialiser la géométrie originale

    def showEvent(self, event):
        super().showEvent(event)
        self.original_geometry = self.geometry()

    def dragEnterEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
            self.setProperty("drag", True)
            self.setStyleSheet(self.styleSheet())
            self.animate_enlarge()
        else:
            event.ignore()

    def dragLeaveEvent(self, event):
        self.setProperty("drag", False)
        self.setStyleSheet(self.styleSheet())
        self.animate_shrink()

    def dragMoveEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
        else:
            event.ignore()

    def dropEvent(self, event: QDropEvent):
        self.setProperty("drag", False)
        self.setStyleSheet(self.styleSheet())
        self.animate_shrink()
        files = [url.toLocalFile() for url in event.mimeData().urls()]
        for file in files:
            print(f"Dropped file: {file}")
            if self.parent.conversion_mode:
                self.parent.convert_image(file)
            else:
                self.parent.append_message("Please select a target format!")
        event.acceptProposedAction()

    def animate_enlarge(self):
        if not self.original_geometry:
            return
        geom = self.original_geometry
        delta_w = int(geom.width() * 0.02)  # Réduction de l'agrandissement à 2%
        delta_h = int(geom.height() * 0.02)
        enlarged_geom = QRect(
            geom.x() - delta_w // 2,
            geom.y() - delta_h // 2,
            geom.width() + delta_w,
            geom.height() + delta_h
        )
        self.animation = QPropertyAnimation(self, b"geometry")
        self.animation.setDuration(200)
        self.animation.setStartValue(self.geometry())
        self.animation.setEndValue(enlarged_geom)
        self.animation.setEasingCurve(QEasingCurve.OutQuad)
        self.animation.start()

    def animate_shrink(self):
        if not self.original_geometry:
            return
        self.animation = QPropertyAnimation(self, b"geometry")
        self.animation.setDuration(200)
        self.animation.setStartValue(self.geometry())
        self.animation.setEndValue(self.original_geometry)
        self.animation.setEasingCurve(QEasingCurve.OutQuad)
        self.animation.start()




class ImageConverterGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Images Converter")
        self.setFixedSize(600, 450)  # Taille réduite d'environ 20%
        self.setWindowIcon(QIcon(icon_path))

        self.conversion_mode = None
        self.compression_level = None  # Initialisation du niveau de compression

        # Palette de couleurs modernes
        self.setStyleSheet("""
            QMainWindow {
                background-color: #0D0D0D;
            }
            QLabel, QRadioButton, QPushButton, QComboBox, QTextEdit {
                color: #0D0D0D;
                font-size: 14px;
            }
            QLabel#SpecialText, QCheckBox#SpecialText {
                color: #F2F2F2;
            }
            #DragDropWidget {
                background-color: #F2F2F2;
                border-radius: 5px;
            }
            #DragDropWidget[drag="true"] {
                background-color: #BFF205;
            }
            #DragDropText {
                color: #0D0D0D;
                font-size: 16px; /* Taille de police réduite */
                font-weight: bold; /* Texte en gras */
            }
            QPushButton {
                background-color: #F2F2F2;
                color: #0D0D0D;
                border: none;
                padding: 5px;
                border-radius: 3px;
            }
            QPushButton:hover {
                background-color: #E0E0E0;
            }
            QPushButton:checked {
                background-color: #BFF205;
                color: #0D0D0D;
            }
            QPushButton#CompressionButton, QPushButton#ConversionButton {
                font-size: 12px;
            }
            QTextEdit {
                background-color: #F2F2F2;
                border: 1px solid #CCCCCC;
                border-radius: 5px;
            }
            /* Styles pour la case à cocher */
            QCheckBox {
                font-size: 14px;
            }
            QCheckBox::indicator {
                width: 20px;
                height: 20px;
            }

            QCheckBox::indicator:unchecked {
                image: url('assets/unchecked_icon.png');
            }

            QCheckBox::indicator:checked {
                image: url('assets/checked_icon.png');
            }
        """)

        # Configuration du layout principal
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(10, 10, 10, 10)
        central_widget = QWidget()
        central_widget.setLayout(main_layout)
        self.setCentralWidget(central_widget)

        # Zone de glisser-déposer
        self.drop_widget = DragDropWidget(self)
        self.drop_widget.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        main_layout.addWidget(self.drop_widget)

        # Espacement
        main_layout.addSpacing(5)

        # Layout des options de conversion et de compression
        options_layout = QHBoxLayout()
        options_layout.setSpacing(20)

        # Options de conversion (à gauche)
        conversion_layout = QVBoxLayout()
        conversion_layout.setSpacing(5)

        conversion_label = QLabel("Convert to format:")
        conversion_label.setObjectName("SpecialText")
        conversion_layout.addWidget(conversion_label)

        # Boutons de conversion
        conversion_buttons_layout = QHBoxLayout()
        self.btn_to_jpeg = QPushButton("JPEG")
        self.btn_to_jpeg.setCheckable(True)
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('jpeg'))
        self.btn_to_jpeg.setFixedSize(60, 25)
        self.btn_to_jpeg.setObjectName("ConversionButton")
        conversion_buttons_layout.addWidget(self.btn_to_jpeg)

        self.btn_to_webp = QPushButton("WEBP")
        self.btn_to_webp.setCheckable(True)
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('webp'))
        self.btn_to_webp.setFixedSize(60, 25)
        self.btn_to_webp.setObjectName("ConversionButton")
        conversion_buttons_layout.addWidget(self.btn_to_webp)

        self.btn_to_png = QPushButton("PNG")
        self.btn_to_png.setCheckable(True)
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('png'))
        self.btn_to_png.setFixedSize(60, 25)
        self.btn_to_png.setObjectName("ConversionButton")
        conversion_buttons_layout.addWidget(self.btn_to_png)

        # Rendre les boutons mutuellement exclusifs
        self.conversion_buttons = [self.btn_to_jpeg, self.btn_to_webp, self.btn_to_png]
        self.button_group = QButtonGroup()
        self.button_group.setExclusive(True)
        for btn in self.conversion_buttons:
            self.button_group.addButton(btn)
            btn.setCheckable(True)
            btn.clicked.connect(self.update_button_styles)
            font = btn.font()
            font.setPointSize(10)
            btn.setFont(font)
            btn.setSizePolicy(QSizePolicy.Fixed, QSizePolicy.Fixed)

        conversion_layout.addLayout(conversion_buttons_layout)
        options_layout.addLayout(conversion_layout)

        options_layout.addStretch()

        # Options de compression (à droite)
        compression_layout = QVBoxLayout()
        compression_layout.setSpacing(5)

        compression_label = QLabel("Compression level:")
        compression_label.setObjectName("SpecialText")
        compression_layout.addWidget(compression_label, alignment=Qt.AlignRight)

        # Boutons de compression
        compression_buttons_layout = QHBoxLayout()
        self.btn_comp_none = QPushButton("NONE")
        self.btn_comp_low = QPushButton("LOW")
        self.btn_comp_medium = QPushButton("MEDIUM")
        self.btn_comp_high = QPushButton("HIGH")

        self.compression_buttons = [self.btn_comp_none, self.btn_comp_low, self.btn_comp_medium, self.btn_comp_high]
        self.compression_group = QButtonGroup()
        self.compression_group.setExclusive(True)

        for btn in self.compression_buttons:
            self.compression_group.addButton(btn)
            btn.setCheckable(True)
            btn.clicked.connect(self.update_compression_styles)
            btn.setFixedSize(60, 25)
            btn.setObjectName("CompressionButton")
            font = btn.font()
            font.setPointSize(10)  # Taille de police légèrement réduite
            btn.setFont(font)
            btn.setSizePolicy(QSizePolicy.Fixed, QSizePolicy.Fixed)
            compression_buttons_layout.addWidget(btn)

        # Définir "NONE" comme sélection par défaut
        self.btn_comp_none.setChecked(True)
        self.update_compression_styles()

        compression_layout.addLayout(compression_buttons_layout)
        options_layout.addLayout(compression_layout)

        main_layout.addLayout(options_layout)

        # Espacement
        main_layout.addSpacing(5)

        # Option pour supprimer l'image originale (alignée à gauche)
        delete_layout = QHBoxLayout()
        #delete_layout.addStretch()

        self.checkbox_delete_original = QCheckBox("Delete original image")
        self.checkbox_delete_original.setChecked(True)
        self.checkbox_delete_original.setObjectName("SpecialText")
        delete_layout.addWidget(self.checkbox_delete_original)

        delete_layout.addStretch()
        main_layout.addLayout(delete_layout)

        # Réduire la hauteur du terminal de messages
        self.message_terminal = QTextEdit()
        self.message_terminal.setReadOnly(True)
        self.message_terminal.setFixedHeight(80)  # Réduit de 120 à 80
        self.message_terminal.setObjectName("MessageTerminal")
        main_layout.addWidget(self.message_terminal)

        self.append_message("Select a conversion format by clicking a button to start.")

    # Les méthodes suivantes restent inchangées
    def append_message(self, message):
        self.message_terminal.append(message)

    def set_conversion_mode_and_clear_message(self, mode):
        self.message_terminal.clear()
        self.set_conversion_mode(mode)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.update_button_styles()
        self.append_message(f"Mode: Convert to {mode.upper()}")

    def update_button_styles(self):
        for btn in self.conversion_buttons:
            if btn.isChecked():
                btn.setStyleSheet("background-color: #BFF205; color: #0D0D0D;")
            else:
                btn.setStyleSheet("background-color: #F2F2F2; color: #0D0D0D;")

    def update_compression_styles(self):
        for btn in self.compression_buttons:
            if btn.isChecked():
                btn.setStyleSheet("background-color: #BFF205; color: #0D0D0D;")
                # Définir le niveau de compression en fonction du bouton sélectionné
                if btn.text() == "NONE":
                    self.compression_level = None
                elif btn.text() == "LOW":
                    self.compression_level = 'low'
                elif btn.text() == "MEDIUM":
                    self.compression_level = 'medium'
                elif btn.text() == "HIGH":
                    self.compression_level = 'high'
            else:
                btn.setStyleSheet("background-color: #F2F2F2; color: #0D0D0D;")

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
                self.append_message(f"Conversion of {file_name} to JPEG impossible: the image contains transparency.")
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
                        if output_path and os.path.exists(output_path):
                            os.remove(output_path)
                else:
                    if not self.checkbox_delete_original.isChecked():
                        base, ext = os.path.splitext(image_path)
                        output_path = f"{base}-clean{ext}"
                        shutil.move(working_image_path, output_path)
                        self.append_message(f"Metadata of {file_name} cleaned. New image created.")
                        new_image_created = True
                        conversion_successful = True
                    else:
                        os.replace(working_image_path, image_path)
                        self.append_message(f"Metadata of {file_name} cleaned.")
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
        return self.compression_level

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
