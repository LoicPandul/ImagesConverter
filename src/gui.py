from PySide6.QtWidgets import (
    QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, QTextEdit,
    QLabel, QCheckBox, QHBoxLayout, QFrame, QSizePolicy, QButtonGroup
)
from PySide6.QtCore import Qt, QUrl, QSize, QPropertyAnimation, QEasingCurve, QRect
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

class DragDropWidget(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.parent = parent

        self.setAcceptDrops(True)
        self.setObjectName("DragDropWidget")
        self.layout = QVBoxLayout()
        self.setLayout(self.layout)

        self.icon_label = QLabel()
        pixmap = QPixmap(image_icon_path).scaled(120, 120, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        self.icon_label.setPixmap(pixmap)
        self.icon_label.setAlignment(Qt.AlignCenter)

        self.text_label = QLabel("Glissez-déposez vos images ici")
        self.text_label.setAlignment(Qt.AlignCenter)
        self.text_label.setObjectName("DragDropText")

        self.layout.addStretch()
        self.layout.addWidget(self.icon_label)
        self.layout.addWidget(self.text_label)
        self.layout.addStretch()

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
            super().dragEnterEvent(event)

    def dragLeaveEvent(self, event):
        self.setProperty("drag", False)
        self.setStyleSheet(self.styleSheet())
        self.animate_shrink()

    def dragMoveEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
        else:
            super().dragMoveEvent(event)

    def dropEvent(self, event: QDropEvent):
        self.setProperty("drag", False)
        self.setStyleSheet(self.styleSheet())
        self.animate_shrink()
        files = [url.toLocalFile() for url in event.mimeData().urls()]
        for file in files:
            print(f"Fichier déposé : {file}")
            if self.parent.conversion_mode:
                self.parent.convert_image(file)
            else:
                self.parent.append_message("Veuillez sélectionner un format cible !")
        event.acceptProposedAction()

    def animate_enlarge(self):
        if not hasattr(self, 'original_geometry'):
            return
        geom = self.original_geometry
        delta_w = int(geom.width() * 0.02)  # Réduction de l'agrandissement à 2%
        delta_h = int(geom.height() * 0.02)
        enlarged_geom = QRect(geom.x() - delta_w // 2, geom.y() - delta_h // 2, geom.width() + delta_w, geom.height() + delta_h)
        self.animation = QPropertyAnimation(self, b"geometry")
        self.animation.setDuration(200)
        self.animation.setStartValue(self.geometry())
        self.animation.setEndValue(enlarged_geom)
        self.animation.setEasingCurve(QEasingCurve.OutQuad)
        self.animation.start()

    def animate_shrink(self):
        if not hasattr(self, 'original_geometry'):
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
        self.setFixedSize(700, 600)
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
                border-radius: 10px;
            }
            #DragDropWidget[drag="true"] {
                background-color: #BFF205;
            }
            #DragDropText {
                color: #0D0D0D;
                font-size: 18px;
            }
            QPushButton {
                background-color: #F2F2F2;
                color: #0D0D0D;
                border: none;
                padding: 10px;
                border-radius: 10px;
            }
            QPushButton:hover {
                background-color: #E0E0E0;
            }
            QPushButton:checked {
                background-color: #BFF205;
                color: #0D0D0D;
            }
            QTextEdit {
                background-color: #F2F2F2;
                border: 1px solid #CCCCCC;
                border-radius: 10px;
            }
            QComboBox {
                background-color: #F2F2F2;
                color: #0D0D0D;
                border: 1px solid #CCCCCC;
                border-radius: 5px;
                padding: 5px;
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
        main_layout.setContentsMargins(20, 20, 20, 20)
        central_widget = QWidget()
        central_widget.setLayout(main_layout)
        self.setCentralWidget(central_widget)

        # Zone de glisser-déposer
        self.drop_widget = DragDropWidget(self)
        main_layout.addWidget(self.drop_widget)

        # Espacement
        main_layout.addSpacing(15)

        # Options de conversion
        options_layout = QHBoxLayout()
        options_layout.setSpacing(20)
        options_layout.addStretch()  # Pour centrer horizontalement

        # Boutons de conversion modernisés
        self.btn_to_jpeg = QPushButton("JPEG")
        self.btn_to_jpeg.setCheckable(True)
        self.btn_to_jpeg.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('jpeg'))
        options_layout.addWidget(self.btn_to_jpeg)

        self.btn_to_webp = QPushButton("WEBP")
        self.btn_to_webp.setCheckable(True)
        self.btn_to_webp.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('webp'))
        options_layout.addWidget(self.btn_to_webp)

        self.btn_to_png = QPushButton("PNG")
        self.btn_to_png.setCheckable(True)
        self.btn_to_png.clicked.connect(lambda: self.set_conversion_mode_and_clear_message('png'))
        options_layout.addWidget(self.btn_to_png)

        # Rendre les boutons mutuellement exclusifs avec QButtonGroup
        self.conversion_buttons = [self.btn_to_jpeg, self.btn_to_webp, self.btn_to_png]
        self.button_group = QButtonGroup()
        self.button_group.setExclusive(True)
        for btn in self.conversion_buttons:
            self.button_group.addButton(btn)
            btn.setCheckable(True)
            btn.clicked.connect(self.update_button_styles)
            btn.setFixedSize(100, 40)  # Même taille pour tous les boutons
            btn.setSizePolicy(QSizePolicy.Fixed, QSizePolicy.Fixed)

        options_layout.addStretch()  # Pour centrer horizontalement
        main_layout.addLayout(options_layout)

        # Espacement
        main_layout.addSpacing(15)

        # Autres options
        other_options_layout = QHBoxLayout()

        # Option "Supprimer l'image originale"
        self.checkbox_delete_original = QCheckBox("Supprimer l'image originale après conversion")
        self.checkbox_delete_original.setChecked(True)
        self.checkbox_delete_original.setObjectName("SpecialText")
        other_options_layout.addWidget(self.checkbox_delete_original)

        other_options_layout.addStretch()

        # Options de compression
        compression_layout = QVBoxLayout()
        compression_label = QLabel("Niveau de compression :")
        compression_label.setObjectName("SpecialText")
        compression_layout.addWidget(compression_label, alignment=Qt.AlignRight)

        # Boutons pour le niveau de compression
        compression_buttons_layout = QHBoxLayout()
        self.btn_comp_none = QPushButton("AUCUNE")
        self.btn_comp_low = QPushButton("FAIBLE")
        self.btn_comp_medium = QPushButton("MOYENNE")
        self.btn_comp_high = QPushButton("FORTE")

        self.compression_buttons = [self.btn_comp_none, self.btn_comp_low, self.btn_comp_medium, self.btn_comp_high]
        self.compression_group = QButtonGroup()
        self.compression_group.setExclusive(True)

        for btn in self.compression_buttons:
            self.compression_group.addButton(btn)
            btn.setCheckable(True)
            btn.clicked.connect(self.update_compression_styles)
            btn.setFixedSize(80, 30)
            btn.setSizePolicy(QSizePolicy.Fixed, QSizePolicy.Fixed)
            compression_buttons_layout.addWidget(btn)

        # Définir "AUCUNE" comme sélection par défaut
        self.btn_comp_none.setChecked(True)
        self.update_compression_styles()

        compression_layout.addLayout(compression_buttons_layout)
        other_options_layout.addLayout(compression_layout)

        main_layout.addLayout(other_options_layout)

        # Terminal de messages
        self.message_terminal = QTextEdit()
        self.message_terminal.setReadOnly(True)
        self.message_terminal.setFixedHeight(150)
        self.message_terminal.setObjectName("MessageTerminal")
        main_layout.addWidget(self.message_terminal)

        self.append_message("Sélectionnez un format de conversion en cliquant sur un bouton pour commencer.")

    def append_message(self, message):
        self.message_terminal.append(message)

    def set_conversion_mode_and_clear_message(self, mode):
        self.message_terminal.clear()
        self.set_conversion_mode(mode)

    def set_conversion_mode(self, mode):
        self.conversion_mode = mode
        self.update_button_styles()
        self.append_message(f"Mode : Convertir en {mode.upper()}")

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
                if btn.text() == "AUCUNE":
                    self.compression_level = None
                elif btn.text() == "FAIBLE":
                    self.compression_level = 'low'
                elif btn.text() == "MOYENNE":
                    self.compression_level = 'medium'
                elif btn.text() == "FORTE":
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
                self.append_message(f"Format de {file_name} non supporté.")
                return
            if self.conversion_mode == 'jpeg' and has_transparency(image_path):
                self.append_message(f"Conversion de {file_name} en JPEG impossible : l'image contient de la transparence.")
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
                        self.append_message(f"{file_name} a été compressé.")
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
                        self.append_message(f"Métadonnées de {file_name} nettoyées. Nouvelle image créée.")
                        new_image_created = True
                        conversion_successful = True
                    else:
                        os.replace(working_image_path, image_path)
                        self.append_message(f"Métadonnées de {file_name} nettoyées.")
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
            self.append_message(f"Une erreur est survenue lors du traitement de {file_name} : {e}")
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
