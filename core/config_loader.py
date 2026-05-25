import commentjson as json

from stickers.text import TextSticker
from stickers.image import ImageSticker
from stickers.clock import ClockSticker
from stickers.api_text import APITextSticker

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

        def get_value(*keys, default=None):
            for key in keys:
                if key in data:
                    return data[key]
            return default

        sticker_type = data["type"]
        css_class = data.get("class", "")
        if sticker_type == "text":

            return TextSticker(
                text=data["text"],
                font_size=get_value("font_size", "font-size", default=32),
                anchor=get_value("anchor", default="top-left"),
                offset_x=get_value("offset_x", "offset-x", default=0),
                offset_y=get_value("offset_y", "offset-y", default=0),
                css_class=css_class
            )

        elif sticker_type == "image":

            return ImageSticker(
                path=data["path"],
                width=data.get("width"),
                height=data.get("height"),
                x=get_value("x", default=0),
                y=get_value("y", default=0),
                anchor=get_value("anchor", default="top-left"),
                offset_x=get_value("offset_x", "offset-x", default=0),
                offset_y=get_value("offset_y", "offset-y", default=0),
                z_index=data.get("z_index", 0),
                css_class=css_class
            )
        elif sticker_type == "clock":
            return ClockSticker(
                format=data.get("format","%H:%M:%S"),
                update_interval=data.get("update_interval",1),
                anchor=get_value("anchor", "top-left"),
                offset_x=get_value("offset_x",0),
                offset_y=get_value("offset_y",0),
                font_size=get_value("font_size", "font-size", default=32),
                css_class=css_class,
                z_index=data.get("z_index", 0)
            )
        elif sticker_type == "api_text":
            return APITextSticker(
                url=data["url"],
                update_interval=data.get("update_interval", 10),
                anchor=data.get("anchor","top-left"),
                offset_x=data.get("offset_x",0),
                offset_y=data.get("offset_y",0),
                font_size=data.get("font_size", 32),
                css_class=css_class,
                z_index=data.get("z_index", 0)
            )

        raise ValueError(
            f"Unknown sticker type: {sticker_type}"
        )