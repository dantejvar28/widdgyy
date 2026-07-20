# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

from stickers.base import Sticker

class VBox(Sticker):
    def __init__(
            self,
            children=None,
            spacing=0,
            padding=0,
            padding_top=None,
            padding_right=None,
            padding_bottom=None,
            padding_left=None,
            justify="start",
            align_items="start",
            **kwargs
    ):
        super().__init__(**kwargs)
        self.children = children or []

        self.spacing = spacing
        self.padding_top = padding if padding_top is None else padding_top
        self.padding_right = padding if padding_right is None else padding_right
        self.padding_bottom = padding if padding_bottom is None else padding_bottom
        self.padding_left = padding if padding_left is None else padding_left
        self.align_items = align_items
        self.justify = justify 

    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        total_height = self.padding_top + self.padding_bottom
        max_width = 0
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                screen_width,
                screen_height
            )

            total_height += (
                child.margin_top + child_h + child.margin_bottom
            )

            max_width = max(
                max_width,
                child_w
            )
        
        if self.children: 
            total_height += (
                len(self.children)-1
            ) * self.spacing 
        
        return (
            max_width + self.padding_left + self.padding_right,
            total_height
        )
    def render(
            self,
            ctx,
            x,
            y,
            w,
            h
    ):
        children_height = 0
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )

            children_height += (
                child.margin_top + child_h + child.margin_bottom
            )
        if self.children:
            children_height += (
                len(self.children)-1
            )*self.spacing

        content_h = h - self.padding_top - self.padding_bottom
        content_w = w - self.padding_left - self.padding_right
        
        if self.justify == "start":
            current_y = y + self.padding_top
        elif self.justify == "center":
            current_y = (
                y + self.padding_top + (content_h - children_height)/2
            )
        elif self.justify == "end":
            current_y = (
                y + h - self.padding_bottom - children_height
            )
        else: 
            current_y = y + self.padding_top

        for child in self.children: 
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )
            if self.align_items == "start": 
                child_x = x + self.padding_left
            elif self.align_items == "center":
                child_x = (
                    x + self.padding_left + (content_w - child_w)/2
                )
            elif self.align_items == "end":
                child_x = (
                    x + w - self.padding_right - child_w
                )
            else: 
                child_x = x + self.padding_left
            
            current_y += child.margin_top
            child.render(
                ctx,
                child_x,
                current_y, 
                child_w,
                child_h
            )
            current_y += (
                child_h + child.margin_bottom + self.spacing
            )  

    def update(self, delta):
        dirty = False
        for child in self.children:
            if child.should_update(delta):
                result = child.update(delta)
                dirty = bool(result) or dirty

            if getattr(child, "consume_dirty", None) and child.consume_dirty():
                dirty = True

        if dirty:
            self.mark_dirty()
        return dirty