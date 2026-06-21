use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use gstreamer as gst;
use gtk4::glib::object::ObjectType;

use crate::state::VideoState;
use crate::pipeline;
use crate::metadata;
use crate::gl_widget::VideoGLWidget;

#[pyclass(unsendable)]
pub struct VideoBackend {

    path: String,

    pipeline: gst::Pipeline,

    state: VideoState,

    loaded: bool,
    gl_widget: VideoGLWidget,
    width: u32,
    height: u32,

    duration: u64,
    current_time: u64,

    fps: f32,

    volume: f32,

    muted: bool,

    looped: bool,
}

#[pymethods]
impl VideoBackend {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        gtk4::init()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to initialize GTK: {e}")))?;

        let pipeline = pipeline::create_pipeline(&path);
        let duration = metadata::duration(&pipeline);
        let gl_widget = VideoGLWidget::new();
        Ok(VideoBackend {
            path,
            pipeline,
            state: VideoState::Stopped,
            loaded: duration > 0,
            gl_widget,
            width: 0,
            height: 0,
            duration,
            current_time: 0,
            looped: false,
            volume: 1.0,
            muted: false,
            fps: 0.0,
        })
    }

    fn play(&mut self) {
        crate::player::play(&self.pipeline);
        self.state = VideoState::Playing;
    }

    fn pause(&mut self) {
        crate::player::pause(&self.pipeline);
        self.state = VideoState::Paused;
    }

    fn stop(&mut self) {
        crate::player::stop(&self.pipeline);
        self.current_time = 0;
        self.state = VideoState::Stopped;
    }

    fn seek(&mut self, ms: u64) {
        crate::player::seek(&self.pipeline, ms);
        self.current_time = ms.min(self.duration);
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
        self.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    #[getter]
    fn duration(&self) -> u64 {
        self.duration
    }

    #[getter]
    fn current_time(&mut self) -> u64 {
        self.current_time = metadata::position(&self.pipeline);
        self.current_time
    }

    #[getter]
    fn fps(&self) -> f32 {
        self.fps
    }

    #[getter]
    fn volume(&self) -> f32 {
        self.volume
    }

    #[getter]
    fn muted(&self) -> bool {
        self.muted
    }

    #[getter]
    fn looped(&self) -> bool {
        self.looped
    }

    #[getter]
    fn widget_ptr(&self) -> usize {
        self.gl_widget
            .widget()
            .as_ptr() as usize
    }

    #[getter]
    fn state(&self) -> String {
        match self.state {
            VideoState::Playing => "playing",
            VideoState::Paused => "paused",
            VideoState::Stopped => "stopped",
        }
        .to_string()
    }

}