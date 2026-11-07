# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
import unittest

from widdgyy import ApiText, Clock, Grid, HBox, Image, Text, VBox, Video


class NodeTests(unittest.TestCase):
    def test_common_fields(self):
        node = Text("hi", cls=["a", "b"], id="x", anchor="center", offset=(1, -2), z=3)
        data = node.to_dict()
        self.assertEqual(data["type"], "text")
        self.assertEqual(data["text"], "hi")
        self.assertEqual(data["class"], "a b")
        self.assertEqual(data["id"], "x")
        self.assertEqual(data["anchor"], "center")
        self.assertEqual((data["offset_x"], data["offset_y"]), (1.0, -2.0))
        self.assertEqual(data["z_index"], 3)

    def test_rejects_unknown_anchor(self):
        with self.assertRaises(ValueError):
            Text("x", anchor="middle")

    def test_container_serializes_children(self):
        node = HBox(Clock("%H:%M", cls="clock"), Text("x"), cls="row")
        data = node.to_dict()
        self.assertEqual(data["type"], "hbox")
        self.assertEqual([child["type"] for child in data["children"]], ["clock", "text"])

    def test_grid_columns(self):
        data = Grid(Text("a"), columns=3).to_dict()
        self.assertEqual(data["columns"], 3)

    def test_api_text_and_media(self):
        api = ApiText(
            "https://example.com",
            template="{t}",
            fields={"t": "a.b"},
            cls="weather",
        ).to_dict()
        self.assertEqual(api["url"], "https://example.com")
        self.assertEqual(api["fields"]["t"], "a.b")
        video = Video("assets/x.mp4", loop=True, width="100%").to_dict()
        self.assertEqual(video["type"], "media")
        self.assertEqual(video["path"], "assets/x.mp4")
        self.assertTrue(video["loop"])

    def test_children_must_be_stickers(self):
        with self.assertRaises(TypeError):
            VBox(Image("a.png"), "not a sticker")


if __name__ == "__main__":
    unittest.main()
