use slint::private_unstable_api::use_24_hour_format;

#[derive(Clone)]
pub struct Options {
    pub(crate) play_speed: f64,
    pub(crate) start_secs: i32,
    pub(crate) end_secs: i32,
    pub(crate) with_audio: bool,
    pub(crate) with_video: bool,
    pub(crate) max_width: i64,
    pub(crate) max_height: i64,
    pub(crate) fixed_lib: usize
}

impl Options {
    pub fn def() -> Self {
        Self {
            play_speed: 1.0,
            start_secs: 0,
            end_secs: 0,
            with_audio: true,
            with_video: true,
            max_width: 1280,
            max_height: 720,
            fixed_lib: usize::MAX,
        }
    }

    pub fn def_audio() -> Self {
        Self {
            play_speed: 1.0,
            start_secs: 0,
            end_secs: 0,
            with_audio: true,
            with_video: false,
            max_width: 0,
            max_height: 0,
            fixed_lib: usize::MAX,
        }
    }

    pub fn def_xxx() -> Self {
        Self {
            play_speed: 1.0,
            start_secs: 180,
            end_secs: -20,
            with_audio: false,
            with_video: true,
            max_width: 640,
            max_height: 360,
            fixed_lib: usize::MAX,
        }
    }

    pub fn fast_end_clip() -> Self {
        Self {
            play_speed: 2.0,
            start_secs: -90,
            end_secs: -20,
            with_audio: false,
            with_video: true,
            max_width: 640,
            max_height: 360,
            fixed_lib: usize::MAX,
        }
    }
}