import commentjson as json

from stickers.text import TextSticker
from stickers.image import ImageSticker


class ConfigLoader:

    def __init__(self, path):

        self.path = path

    def load_scene(self, scene):

        with open(self.path) as f:

            config = json.load(f)

        for sticker_data in config["stickers"]:

            sticker = self.create_sticker(
                sticker_data
            )

            scene.add(sticker)

    def create_sticker(self, data):

        sticker_type = data["type"]
        css_class = data.get("class", "")
        if sticker_type == "text":

            return TextSticker(
                text=data["text"],
                x=data["x"],
                y=data["y"],
                font_size=data.get(
                    "font_size",
                    32
                ),
                css_class=css_class
            )

        elif sticker_type == "image":

            return ImageSticker(
                path=data["path"],
                x=data["x"],
                y=data["y"]
            )

        raise ValueError(
            f"Unknown sticker type: {sticker_type}"
        )