use pyo3::prelude::*;
use gstreamer as gst;
use gst::prelude::*;
use gstreamer_pbutils::{Discoverer,DiscovererInfo};
use gstreamer_app::AppSink;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VideoState {
    Playing,
    Paused,
    Stopped,
}

#[pyclass]
struct VideoBackend {
    path:String,
    state:VideoState,

    pipeline: Option<gst::Pipeline>,
    appsink: Option<gst_app::AppSink>,

    loaded: bool,

    width:u32,
    height:u32,
    frame: Option<gst::Buffer>,

    duration:u64,
    current_time:u64,

    looped:bool,
    volume:f32,
    muted:bool,

    fps:f32,    
}


#[pymethods]
impl VideoBackend {
    #[new]
    fn new(path: String) -> Self {
        gst::init().unwrap();
        let uri = glib::filename_to_uri(
            std::fs::canonicalize(&path).unwrap(),
            None,
        )
        .unwrap();
        
        let pipeline = gst::parse::launch(
            &format!(
                "filesrc location=\"{}\" !\
                decodebin !\
                videoconvert !\
                video\x-raw,format=BGRA !\
                appsink name=sink",
                path
            )
        ).unwrap()
        let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();

        let appsink = pipeline 
            .by_name("sink")
            .unwrap()
            .downcast::<AppSink>()
            .unwrap();

        let bus = pipeline.bus().unwrap();
        let _ =bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(5),
            &[gst::MessageType::AsyncDone, gst::MessageType::Error],
        );
        

        let duration = pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.mseconds())
            .unwrap_or(0);
        let mut loaded = duration > 0;
        let mut width = 0;
        let mut height = 0;
        let mut fps = 0.0;
        
        VideoBackend {
            path,
            state: VideoState::Stopped,
            pipeline: Some(pipeline),   
            loaded, 
            width,
            height,
            duration,
            current_time: 0,
            
            looped: false,
            volume: 1.0,
            muted: false,
            fps,
        }
    }
// -----------Getters ------------
    #[getter]
    fn looped(&self) -> bool{
        self.looped
    }

    #[getter]
    fn muted(&self) -> bool{
        self.muted
    }

    #[getter]
    fn volume(&self) -> f32{
        self.volume
    }

    #[getter]
    fn loaded(&self) -> bool{
        self.loaded
    }
    #[getter]
    fn duration(&self) -> u64{
        self.duration
    }

    #[getter]
    fn current_time(&self) -> u64{
        self.current_time
    }

    #[getter]
    fn width(&self) -> u32{
        self.width
    }

    #[getter]
    fn height(&self) -> u32{
        self.height
    }

    #[getter]
    fn fps(&self) -> f32{
        self.fps
    }

    #[getter]
    fn playing(&self) -> bool{
        matches!(self.state, VideoState::Playing)
    }

    #[getter]
    fn path(&self) -> &str{
        &self.path
    }

    #[getter]
    fn state(&self) -> String{
        match self.state {
            VideoState::Playing => "playing",
            VideoState::Paused => "paused",
            VideoState::Stopped => "stopped",
        }.to_string()
    }
// -----------Setters ------------

    #[setter]
    fn set_volume(&mut self, volume: f32){
        self.volume = volume.clamp(0.0,1.0);
    }

    #[setter]
    fn set_muted(&mut self, muted:bool){
        self.muted = muted;
    }

    #[setter]
    fn set_looped(&mut self, looped:bool){
        self.looped = looped;
    }

// -----------Methods ------------    
    fn seek(&mut self, ms: u64){
        if let Some(pipeline) = &self.pipeline{
            pipeline.seek_simple(
                gst::SeekFlags::FLUSH,
                gst::ClockTime::from_mseconds(ms),
            );
        }
        self.current_time = ms.min(self.duration);
    }
    fn close (&mut self){
        if let Some(pipeline) = &self.pipeline{
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.pipeline = None;
        self.state = VideoState::Stopped;
        self.loaded = false;
        
        self.width = 0;
        self.height = 0;

        self.duration = 0;
        self.current_time = 0;
    }

    fn play(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            let _ = pipeline.set_state(gst::State::Playing);
        }
        self.state = VideoState::Playing;
    }

    fn pause(&mut self) {
        if let Some(pipeline) = &self.pipeline{
            let _ = pipeline.set_state(gst::State::Paused);
        }
        self.state = VideoState::Paused;
    }

    fn stop(&mut self) {
        if let Some(pipeline) = &self.pipeline{
            let _ = pipeline.set_state(gst::State::Ready);

            let _ = pipeline.seek_simple(
                gst::SeekFlags::FLUSH,
                gst::ClockTime::ZERO,
            );
        }
        self.current_time = 0;
        self.state = VideoState::Stopped;
        
    }

    fn update (&mut self, _delta:f64){
        if let Some(appsink)=&self.appsink{
            if let Some(sample) = appsink.try_pull_sample(gst::ClockTime::ZERO){
                let buffer = sample.buffer().unwrap();
                let map = buffer.map_readable().unwrap();
                let pixels = map.as_slice();
            }
        }
    }

    fn render(
        &mut self,
        widget_ptr:usize,
        x:f32,
        y:f32,
        w:f32,
        h:f32,
    ){
        
    }

}

#[pymodule]
fn widdgyy_video(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VideoBackend>()?;
    Ok(())
}
