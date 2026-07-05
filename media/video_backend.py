import sys
from pathlib import Path

import gi
import gi._gi

from media.base_backend import MediaBackend


ROOT = Path(__file__).resolve().parent.parent
NATIVE_VIDEO_DIR = ROOT / "native" / "video"

if str(NATIVE_VIDEO_DIR) not in sys.path:
    sys.path.insert(0, str(NATIVE_VIDEO_DIR))

try:
    import widdgyy_video
except ImportError:
    widdgyy_video = None


def _wrap_native_widget(ptr):
    return gi._gi.pygobject_new_full(ptr, False)


class NativeVideoBackend(MediaBackend):
    def __init__(self, path, autoplay=True, loop=True):
        if widdgyy_video is None:
            raise RuntimeError(
                "Native video backend is not available. Build native/video/widdgyy_video.so first."
            )

        video_path = str(Path(path).expanduser().resolve())
        self._backend = widdgyy_video.VideoBackend(video_path)
        self._widget = _wrap_native_widget(self._backend.widget_ptr)
        self._overlay = None
        self._attached = False

        self._backend.set_looped(loop)

        if autoplay:
            self.play()

        self._loop_threshold_ms = 120

    def attach_overlay(self, overlay):
        if self._overlay is overlay and self._attached:
            return

        if self._overlay is not None and self._attached:
            self._overlay.remove_overlay(self._widget)

        overlay.add_overlay(self._widget)
        self._overlay = overlay
        self._attached = True
        self.show()

    def widget(self):
        return self._widget

    def detach_overlay(self):
        if self._overlay is None or not self._attached:
            return

        self._overlay.remove_overlay(self._widget)
        self._overlay = None
        self._attached = False

    def hide(self):
        self._backend.hide()

    def play(self):
        self._backend.play()

    def pause(self):
        self._backend.pause()

    def stop(self):
        self._backend.stop()

    def update(self, delta):
        # GTK loop mode can be inconsistent on some systems. Keep a Python-side
        # fallback that restarts playback when the stream reaches EOS.
        try:
            if not self._backend.looped:
                return False

            duration = self._backend.duration
            if duration <= 0:
                return False

            current = self._backend.current_time
            is_at_end = current >= max(0, duration - self._loop_threshold_ms)
            if is_at_end:
                self._backend.seek(0)
                self._backend.play()
                return True
        except Exception:
            # Backend state probes should never break the render/update loop.
            return False

        return False

    def measure(self, ctx, screen_width, screen_height):
        return self._backend.width, self._backend.height

    def render(self, ctx, x, y, w, h):
        self._backend.set_bounds(int(x), int(y), max(1, int(w)), max(1, int(h)))

    def destroy(self):
        self.stop()
        self.detach_overlay()


VideoBackend = NativeVideoBackend

