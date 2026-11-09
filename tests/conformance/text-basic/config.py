from widdgyy import HBox, Text

stickers = [
    HBox(
        Text("Hello Widdgyy", cls="title", id="title"),
        Text(
            "The quick brown fox jumps over the lazy dog and keeps running for a while",
            cls="wrap",
            id="wrap",
        ),
        Text("SPACED TEXT", cls="spaced", id="spaced"),
        Text("Right aligned line", cls="right", id="right"),
        cls="root",
        id="root",
    ),
]
