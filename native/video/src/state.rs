#[derive(Clone,Copy,PartialEq,Eq)]
pub enum VideoState {
    Playing,
    Paused,
    Stopped,
}