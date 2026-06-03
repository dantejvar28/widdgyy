from stickers.base import Sticker

class Grid(Sticker):
    
    def __init__(
            self,
            children=None,
            columns=2,
            spacing=0,
            padding=0,
            **kwargs
    ):
        super().__init__(**kwargs)
        self.children=children or []
        
        self.columns=max(1,columns)

        self.spacing=spacing
        self.padding=padding

    def _calculate_grid(
            self,
            ctx,
            width,
            height
    ):
        column_widths=[0] * self.columns
        row_heights=[]

        for i, child in enumerate(self.children):
            row = i // self.columns
            col = i % self.columns
            child_w , child_h = child.measure(
                ctx,
                width,
                height
            )
            column_widths[col]=max(
                column_widths[col],
                child_w
            )
            while len(row_heights) <= row:
                row_heights.append(0)
            row_heights[row]=max(
                row_heights[row],
                child_h
            )

        return column_widths, row_heights

    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        column_widths, row_heights = self._calculate_grid(
            ctx,
            screen_width,
            screen_height
        )
        row_spacing=max(
            0,
            len(row_heights)-1
        )* self.spacing

        column_spacing=max(
            0,
            self.columns-1
        )* self.spacing

        total_width = (sum(column_widths) + column_spacing + self.padding * 2)
        total_height= (sum(row_heights) + row_spacing + self.padding * 2)

        return total_width,total_height

    def render(
            self,
            ctx,
            x,
            y,
            w,
            h
    ):
        column_widths, row_heights = self._calculate_grid(
            ctx,
            w,
            h
        )
        
        for i, child in enumerate(self.children):
            row = i // self.columns 
            col = i % self.columns
            child_x=(
                x + self.padding + sum(column_widths[:col]) + col * self.spacing
            )
            child_y=(
                y + self.padding + sum(row_heights[:row]) + row * self.spacing
            )
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )
            child.render(
                ctx,
                child_x,
                child_y,
                child_w,
                child_h
            )

    def update(self,delta):
        for child in self.children:
            child.update(delta)