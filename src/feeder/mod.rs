use std::collections::HashMap;
use std::fmt::format;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::thread::spawn;
use std::{fs,process};
use  std::ptr::copy;
use rand::Rng;
use crate::helpers::*;
use crate::*;

include!("feeder.rs");