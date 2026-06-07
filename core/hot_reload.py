import os 
import time 

from gi.repository import GLib 

class HotReload:
    def __init__(
            self,
            scene,
            config_loader,
            css_loader,
    ):
        self.scene = scene 
        self.config_loader = config_loader
        self.css_loader = css_loader

        self.config_mtime = 0
        self.css_mtime = 0 
        self.is_reloading = False

    def apply_styles(self, sticker):
        sticker.style = self.css_loader.get_style(
            sticker.css_class
        )

        children = getattr(sticker, "children", None)
        if children:
            for child in children:
                self.apply_styles(child)

    def start(self):
        # Initialize mtimes to avoid an immediate reload on startup.
        self.config_mtime = self._safe_mtime(
            self.config_loader.path
        )
        self.css_mtime = self._safe_mtime(
            self.css_loader.path
        )

        GLib.timeout_add(
            1000,
            self.check_changes
        )

    def _safe_mtime(self, path):
        try:
            return os.path.getmtime(path)
        except OSError:
            return 0

    def check_changes(self):
        config_time = self._safe_mtime(
            self.config_loader.path
        )

        css_time = self._safe_mtime(
            self.css_loader.path
        )

        changed = False

        if config_time != self.config_mtime:
            self.config_mtime = config_time
            changed = True
        if css_time != self.css_mtime:
            self.css_mtime = css_time
            changed = True
        if changed:
            self.reload()
        return True

    def reload(self):
        if self.is_reloading:
            return

        self.is_reloading = True
        print("Reloading...")
        try:
            self.scene.clear()

            self.config_loader.load_scene(
                self.scene
            )

            self.css_loader.load()
            for sticker in self.scene.stickers:
                self.apply_styles(sticker)
        except Exception as err:
            print(f"HotReload error: {err}")
        finally:
            self.is_reloading = False