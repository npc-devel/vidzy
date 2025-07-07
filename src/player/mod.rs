use std::fmt::Debug;
use std::sync::atomic::compiler_fence;
use std::sync::mpsc::channel;
use std::thread;

use slint::{ComponentHandle, Weak};
use std::pin::Pin;
use bytemuck::Pod;
use futures::future::OptionFuture;
use std::future::Future;
use std::path::PathBuf;
use futures::{FutureExt};
use crate::{player, App};
use crate::feeder::*;
use std::os::raw::{c_ulong, c_void};
use derive_more::Display;
use crate::*;

const CPAL_BUFFER_SIZE: usize = 8*1024*1024;
const VIDEO_Q_SIZE: usize = 128;
const AUDIO_Q_SIZE: usize = 32;

#[derive(Clone, Copy, PartialEq, Display, PartialOrd)]
pub enum PlayerCommand {
    None,
    Sleep,
    Die,
    Play,
    Pause,
    Reset,
    Skip_F,
    Skip_R,
    Clone_4,
    More,
    Diff,
    WakeTo0 = 50,
    WakeTo1 = 51,
    WakeTo2 = 52,
    WakeTo3 = 53,
    WakeTo4 = 54,
    WakeTo5 = 55,
    WakeTo6 = 56,
    WakeTo7 = 57,
    WakeTo8 = 58,
    WakeTo9 = 59
}

#[derive(Clone, Copy, PartialEq)]
pub enum PlayerState {
    Sleeping,
    Normal
}


include!("options.rs");
include!("funcs.rs");
//include!("command.rs");
//include!("ffmpeg_player.rs");
include!("mpv_player.rs");