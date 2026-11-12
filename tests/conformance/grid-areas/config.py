from widdgyy import Grid, HBox

stickers = [
    Grid(
        HBox(cls="header", id="header"),
        HBox(cls="side", id="side"),
        HBox(cls="main", id="main"),
        columns=3,
        cls="root",
        id="root",
    ),
]
