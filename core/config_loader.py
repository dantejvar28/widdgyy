import commentjson as json

from stickers.text import TextSticker
from stickers.image import ImageSticker
from stickers.clock import ClockSticker
from stickers.api_text import APITextSticker
from layouts.hbox import Hbox
from layouts.vbox import VBox
from layouts.grid import Grid

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

    def get_margin_kwargs(self,data):
        return{
            "margin": data.get("margin",0),
            "margin_top": data.get("margin_top"),
            "margin_right": data.get("margin_right"),
            "margin_bottom": data.get("margin_bottom"),
            "margin_left": data.get("margin_left")
        }
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
                **self.get_margin_kwargs(data),
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
                **self.get_margin_kwargs(data),
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
                **self.get_margin_kwargs(data),
                anchor=get_value("anchor", default="top-left"),
                offset_x=get_value("offset_x", default=0),
                offset_y=get_value("offset_y", default=0),
                font_size=get_value("font_size", "font-size", default=32),
                css_class=css_class,
                z_index=data.get("z_index", 0)
            )
        elif sticker_type == "api_text":
            return APITextSticker(
                url=data["url"],
                method=data.get("method", "GET"),
                headers=data.get("headers"),
                template=data.get("template",""),
                fields=data.get("fields",{}),
                timeout=data.get("timeout", 10),
                retries=data.get("retries", 1),
                update_interval=data.get("update_interval", 10),
                **self.get_margin_kwargs(data),
                anchor=get_value("anchor", default="top-left"),
                offset_x=get_value("offset_x", default=0),
                offset_y=get_value("offset_y", default=0),
                font_size=get_value("font_size", "font-size", default=32),
                css_class=css_class,
                z_index=data.get("z_index", 0)
            )
        elif sticker_type == "hbox":
            children=[]
            for child_data in data.get("children",[]):
                child = self.create_sticker(
                    child_data
                )
                children.append(child)
            return Hbox(
                children=children,
                spacing=data.get("spacing",0),
                justify=get_value("justify", "align", default="start"),
                align_items=data.get("align_items","start"),
                **self.get_margin_kwargs(data),
                padding=data.get("padding",0),

                padding_top=data.get("padding_top"),
                padding_right=data.get("padding_right"),
                padding_bottom=data.get("padding_bottom"),
                padding_left=data.get("padding_left"),

                anchor=data.get(
                    "anchor","top-left"
                ),
                offset_x=data.get(
                    "offset_x",0
                ),
                offset_y=data.get(
                    "offset_y",0
                ),
                z_index=data.get(
                    "z_index",0
                )
            )
        elif sticker_type == "vbox":
            children=[]
            for child_data in data.get(
                "children",[]
            ):
                child = self.create_sticker(
                    child_data
                )
                children.append(child)
            return VBox(
                children=children,
                spacing=data.get(
                    "spacing",0
                ),
                justify=data.get(
                    "justify","start"
                ),
                align_items=data.get(
                    "align_items","start"
                ),
                **self.get_margin_kwargs(data),
                padding=data.get("padding",0),

                padding_top=data.get("padding_top"),
                padding_right=data.get("padding_right"),
                padding_bottom=data.get("padding_bottom"),
                padding_left=data.get("padding_left"),
                
                anchor=data.get(
                    "anchor","top-left"
                ),
                offset_x=data.get(
                    "offset_x",0
                ),
                offset_y=data.get(
                    "offset_y",0
                ),
                z_index=data.get(
                    "z_index",0
                )

            )
        elif sticker_type == "grid":
            children=[]
            for child_data in data.get("children",[]):
                children.append(
                    self.create_sticker(child_data)
                )
            return Grid(
                children=children,

                columns=data.get("columns",2),
                spacing=data.get("spacing",0),
                padding=data.get("padding",0),

                anchor=data.get(
                    "anchor","top-left"
                ),
                offset_x=data.get(
                    "offset_x",0
                ),
                offset_y=data.get(
                    "offset_y",0
                ),
                z_index=data.get(
                    "z_index",0
                )
            )

        raise ValueError(
            f"Unknown sticker type: {sticker_type}"
        )