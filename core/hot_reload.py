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

    def apply_styles(self, sticker):
        sticker.style = self.css_loader.get_style(
            sticker.css_class
        )

        children = getattr(sticker, "children", None)
        if children:
            for child in children:
                self.apply_styles(child)

    def start(self):
        GLib.timeout_add(
            1000,
            self.check_changes
        )

    def check_changes(self):
        config_time=os.path.getmtime(
            self.config_loader.path
        )

        css_time = os.path.getmtime(
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
        print("Reloading...")
        self.scene.clear()

        self.config_loader.load_scene(
            self.scene
        )

        self.css_loader.load()
        for sticker in self.scene.stickers:
            self.apply_styles(sticker)