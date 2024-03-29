from .gui import ImageConverterGUI
from PySide6.QtWidgets import QApplication
import sys


def main():
    app = QApplication(sys.argv)
    window = ImageConverterGUI()
    window.show()
    sys.exit(app.exec_())
