from media.base_backend import MediaBackend
from gi.repository import Gst
import cairo
from pathlib import Path
import threading


class VideoBackend(MediaBackend):
    def __init__(
            self, 
            path,
            autoplay=True,
            loop=True
    ):
        Gst.init(None)
        self.path = path
        self.loop = loop
        self.autoplay = autoplay

        self.buffer=None
        self.surface=None
        self.width=0
        self.height=0
        self._surface_lock = threading.Lock()
        self._pending_frame = None
        self._has_pending=False
        self._buffer_size = 0
        self._pending_width = 0
        self._pending_height = 0

        self.pipeline = None
        self.appsink = None

        self._playing=False
        self.create_pipeline()

        if self.autoplay:
            self.play()

    def create_pipeline(self):
        video_path = str(Path(self.path).expanduser().resolve())
        video_uri = Gst.filename_to_uri(video_path)
        pipeline = f"""
            uridecodebin uri="{video_uri}" !
            videoconvert !
            video/x-raw,format=BGRA !
            appsink
                name=sink
                emit-signals=true
                max-buffers=1
                drop=true
                sync=true
            """
        self.pipeline = Gst.parse_launch(pipeline)
        self.appsink = self.pipeline.get_by_name("sink")

        bus = self.pipeline.get_bus()
        if bus is not None:
            bus.add_signal_watch()
            bus.connect("message", self.on_bus_message)

        self.appsink.connect(
            "new-sample",
            self.on_new_sample
        )
        self._buffer_size = 0

    def on_bus_message(self, bus, message):
        message_type = message.type

        if message_type == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print(f"VideoBackend error: {err}")
            if debug:
                print(f"VideoBackend debug: {debug}")
            self._playing = False
        elif message_type == Gst.MessageType.EOS:
            if self.loop:
                self.pipeline.seek_simple(
                    Gst.Format.TIME,
                    Gst.SeekFlags.FLUSH | Gst.SeekFlags.KEY_UNIT,
                    0
                )
            else:
                self._playing = False

    def play(self):
        if not self._playing:
            self.pipeline.set_state(Gst.State.PLAYING)
            self._playing=True

    def pause(self):
        self.pipeline.set_state(Gst.State.PAUSED)
        self._playing=False

    def stop(self):
        self.pipeline.set_state(Gst.State.NULL)
        self._playing=False

    def update(self, delta):
        pass

    def on_new_sample(self, sink):
        sample = sink.emit("pull-sample")
        if sample is None:
            return Gst.FlowReturn.ERROR

        buffer = sample.get_buffer()
        caps = sample.get_caps()
        structure = caps.get_structure(0)
        width = structure.get_value("width")
        height = structure.get_value("height")

        if not width or not height:
            return Gst.FlowReturn.OK

        success, mapinfo = buffer.map(Gst.MapFlags.READ)
        if success:
            frame_size = width * height * 4

            # Copiar FUERA del lock — es lo más lento
            local_buf = bytearray(frame_size)
            memoryview(local_buf)[:] = memoryview(mapinfo.data)[:frame_size]
            buffer.unmap(mapinfo)

            with self._surface_lock:
                self._pending_frame = local_buf
                self._pending_width = width
                self._pending_height = height
                self._has_pending = True
        
        return Gst.FlowReturn.OK

      
    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        return self.width, self.height
    
    def render(self, ctx, x, y, w, h):
        with self._surface_lock:
            if self._has_pending and self._pending_frame is not None:
                self.width = self._pending_width
                self.height = self._pending_height
                self.buffer = self._pending_frame
                self._has_pending = False 

                self.surface = cairo.ImageSurface.create_for_data(
                    self.buffer,
                    cairo.FORMAT_ARGB32,
                    self.width,
                    self.height,
                    self.width * 4
                )

            surface = self.surface
            source_w = self.width
            source_h = self.height

        if surface is None or source_w <= 0 or source_h <= 0:
            return

        ctx.save()
        ctx.translate(x, y)
        ctx.scale(w / source_w, h / source_h)
        ctx.set_source_surface(surface, 0, 0)
        ctx.paint()
        ctx.restore()
        
