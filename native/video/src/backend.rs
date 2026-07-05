use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use gtk4::prelude::*;
use gtk4::glib::object::ObjectType;
use gtk4::MediaFile;
use std::path::Path;

use crate::state::VideoState;
use crate::video_widget::VideoWidget;

fn micros_to_millis(value: i64) -> u64 {
    value.max(0) as u64 / 1_000
}

fn millis_to_micros(value: u64) -> i64 {
    value.saturating_mul(1_000).min(i64::MAX as u64) as i64
}

#[pyclass(unsendable)]
pub struct VideoBackend {

    path: String,

    media_file: MediaFile,

    state: VideoState,

    loaded: bool,
    video_widget: VideoWidget,
}

#[pymethods]
impl VideoBackend {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        gtk4::init()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to initialize GTK: {e}")))?;

        if !Path::new(&path).exists() {
            return Err(PyRuntimeError::new_err(format!(
                "Video file does not exist: {path}"
            )));
        }

        let media_file = MediaFile::for_filename(&path);
        // Fallback for environments where loop property alone does not restart playback on EOS.
        media_file.connect_ended_notify(|media_file| {
            if media_file.is_loop() && media_file.is_ended() {
                media_file.seek(0);
                media_file.play();
            }
        });
        let video_widget = VideoWidget::new(&media_file);

        Ok(VideoBackend {
            path,
            media_file,
            state: VideoState::Stopped,
            loaded: true,
            video_widget,
        })
    }

    fn play(&mut self) {
        self.media_file.play();
        self.state = VideoState::Playing;
    }

    fn pause(&mut self) {
        self.media_file.pause();
        self.state = VideoState::Paused;
    }

    fn stop(&mut self) {
        self.media_file.pause();
        self.media_file.seek(0);
        self.state = VideoState::Stopped;
    }

    fn seek(&mut self, ms: u64) {
        self.media_file.seek(millis_to_micros(ms));
    }

    fn show(&self) {
        self.video_widget.show();
    }

    fn hide(&self) {
        self.video_widget.hide();
    }

    fn set_bounds(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.video_widget.set_bounds(x, y, width, height);
    }

    fn set_volume(&self, volume: f32) {
        self.media_file.set_volume(volume.max(0.0) as f64);
    }

    fn set_muted(&self, muted: bool) {
        self.media_file.set_muted(muted);
    }

    fn set_looped(&self, looped: bool) {
        self.media_file.set_loop(looped);
        if looped && self.media_file.is_ended() {
            self.media_file.seek(0);
            self.media_file.play();
        }
    }
    
//  -----------------------Getters for python--------------------------
    #[getter]
    fn path(&self) -> String {
        self.path.clone()
    }

    #[getter]
    fn loaded(&self) -> bool {
        self.loaded
    }

    #[getter]
    fn width(&self) -> u32 {
        self.media_file.intrinsic_width().max(0) as u32
    }

    #[getter]
    fn height(&self) -> u32 {
        self.media_file.intrinsic_height().max(0) as u32
    }

    #[getter]
    fn duration(&self) -> u64 {
        micros_to_millis(self.media_file.duration())
    }

    #[getter]
    fn current_time(&self) -> u64 {
        micros_to_millis(self.media_file.timestamp())
    }

    #[getter]
    fn fps(&self) -> f32 {
        0.0
    }

    #[getter]
    fn volume(&self) -> f32 {
        self.media_file.volume() as f32
    }

    #[getter]
    fn muted(&self) -> bool {
        self.media_file.is_muted()
    }

    #[getter]
    fn looped(&self) -> bool {
        self.media_file.is_loop()
    }

    #[getter]
    fn widget_ptr(&self) -> usize {
        self.video_widget
            .widget()
            .as_ptr() as usize
    }

    #[getter]
    fn state(&self) -> String {
        let state = if self.media_file.is_playing() {
            VideoState::Playing
        } else if self.media_file.timestamp() > 0 {
            VideoState::Paused
        } else {
            self.state
        };

        match state {
            VideoState::Playing => "playing",
            VideoState::Paused => "paused",
            VideoState::Stopped => "stopped",
        }
        .to_string()
    }

}