import sys
from .application import PreludeApplication

def main() -> int:
    app = PreludeApplication()
    return app.run(sys.argv)
