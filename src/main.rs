// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
slint::include_modules!();

mod player;
mod feeder;
mod helpers;
mod scene;
mod input;

use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt::format;
use std::io::ErrorKind::OutOfMemory;
use std::ops::Add;
use std::os::raw::c_void;
use std::thread;
use std::time::Duration;
use derive_more::{Deref,DerefMut};
use glow::{Context, HasContext};
use helpers::*;
use crate::feeder::Feeder;
use crate::player::{MpvPlayer as Player,MpvShared as PlayerShared,MpvStatics as PlayerStatics,*};
use crate::input::*;
use crate::scene::*;

//const ROOT_PATH: &str = "/etc/vidzy";///home/sif/Dev/github/vidzy";
const ROOT_PATH: &str = "/home/doc/Dev/github/vidzy";

const QMODE: usize = 0;
const MAIN_LIB_LAST: usize = 2;
const MAIN_LIB: usize = 1;
static mut PLAYER_STATICS: [PlayerStatics;50] = [PlayerStatics {
    lib: 0,
    show_idx: 0,
    file_idx: 0,
};50];
static mut APP_CONSTS: [usize;50] = [0;50];

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    app_seed();
    thread::scope(|s| {
        let app = App::new().unwrap();
        let app_weak = app.as_weak();
        //let mut ml = args.get(1).unwrap_or(&String::new()).clone();
        //if ml.len()==0 { ml = "tv".to_string(); }
        unsafe  {
            APP_CONSTS[MAIN_LIB] = 0;
            APP_CONSTS[MAIN_LIB_LAST] = usize::MAX;
        }
        let cfg = _conf("player");
        let feed = Feeder::new(&cfg);
        let mut players = HashMap::new();
        let mut senders = HashMap::new();
        let mut rco: Option<PlayerShared> = None;

        let mut fids = vec!["full"];
        let mut qids = vec!["silent_1","silent_2","silent_3","silent_4","tiny"];

        if let Err(error) = app
            .window()
            .set_rendering_notifier(move |state, graphics_api| {
                let wh = if let Some(app) = app_weak.upgrade() {
                    let sf = app.window().scale_factor();
                    let phys_width = sf*app.get_window_width();
                    let phys_height = sf*app.get_window_height();
                    (phys_width,phys_height)
                } else {
                    (1280.0,720.0)
                };

                match state {
                    slint::RenderingState::RenderingSetup => {
                        println!("API: {:?}", graphics_api);
                        let (context, proc_addr) = match graphics_api {
                            slint::GraphicsAPI::NativeOpenGL { get_proc_address } => unsafe {
                                (
                                    glow::Context::from_loader_function(|s| get_proc_address(CStr::from_ptr(s.as_ptr() as *const _))),
                                    get_proc_address,
                                )
                            },
                            _ => return,
                        };
                        let p_addr = proc_addr as *const _ as *mut c_void;
                        let rc = PlayerShared::new(context, p_addr, wh);

                        let odef = Options::def();
                        let adef = Options::def_audio();
                        let oxxx = Options::def_xxx();
                        let oclip = Options::fast_end_clip();


                        let p = Player::run(&rc, feed.clone(), 0, &odef);
                        senders.insert("full".to_string(), p.control_sender.clone());
                        players.insert("full".to_string(), p);

                        for k in qids.iter() {
                            if *k=="silent_4" {
                                let p = Player::run(&rc, feed.clone(), 4, &oclip);
                                senders.insert(k.to_string(), p.control_sender.clone());
                                players.insert(k.to_string(), p);
                            } else if *k=="tiny" {
                                let mut fdef = adef.clone();
                                fdef.fixed_lib = 5;
                                let p = Player::run(&rc, feed.clone(), 5, &fdef);
                                senders.insert(k.to_string(), p.control_sender.clone());
                                players.insert(k.to_string(), p);
                            } else {
                                let i = usize::from_str_radix(&k[7..],10).unwrap();
                                let p = Player::run(&rc, feed.clone(), i, &oxxx);
                                senders.insert(k.to_string(), p.control_sender.clone());
                                players.insert(k.to_string(), p);
                            }
                        }
                        rco = Some(rc);
                    }
                    slint::RenderingState::BeforeRendering => {
                        if let Some(app) = app_weak.upgrade() {
                            if unsafe { APP_CONSTS[MAIN_LIB_LAST] != APP_CONSTS[MAIN_LIB] } {
                                unsafe { APP_CONSTS[MAIN_LIB_LAST] = APP_CONSTS[MAIN_LIB] };
                                let qm = unsafe { APP_CONSTS[QMODE] != 0 };
                                app.set_qmode(qm);

                                let itemi = unsafe { APP_CONSTS[MAIN_LIB] };
                                let mut wcv = PlayerCommand::WakeTo0;
                                match itemi {
                                    1=>{ wcv = PlayerCommand::WakeTo1; }
                                    2=>{ wcv = PlayerCommand::WakeTo2; }
                                    3=>{ wcv = PlayerCommand::WakeTo3; }
                                    4=>{ wcv = PlayerCommand::WakeTo4; }
                                    5=>{ wcv = PlayerCommand::WakeTo5; }
                                    6=>{ wcv = PlayerCommand::WakeTo6; }
                                    7=>{ wcv = PlayerCommand::WakeTo7; }
                                    8=>{ wcv = PlayerCommand::WakeTo8; }
                                    9=>{ wcv = PlayerCommand::WakeTo9; }
                                    _ => {}
                                }

                                let mut qc = wcv;
                                let mut fc = PlayerCommand::Sleep;

                                if !qm {
                                    qc = PlayerCommand::Sleep;
                                    fc = wcv;
                                }

                                for id in qids.iter() {
                                 //   println!("Sending {qc} to {id}");
                                    let rs = senders.get(*id);
                                    if rs.is_some() {
                                        let cs = rs.unwrap();
                                        cs.send_blocking(qc).unwrap_or_default();
                                    };
                                }
                                for id in fids.iter() {
                                   // println!("Sending {fc} to {id}");
                                    let rs = senders.get(*id);
                                    if rs.is_some() {
                                        let cs = rs.unwrap();
                                        cs.send_blocking(fc).unwrap_or_default();
                                    };
                                }
                            }
                            let rc: &mut MpvShared = rco.as_mut().unwrap();
                            let sd = senders.clone();
                            let fd = feed.clone();

                            //let mut emgr = ManagedInput::new(&cfg);
                             app.on_ev_call(move |id,kind,button,mx,my| {
                                 let sid = id.to_string();
                                 println!("Engine ev: {sid} {kind} {button} {mx} {my}");
                                 match kind.to_string().as_str() {
                                     "up" => {
                                         println!("Engine ev: {sid} {kind} {button} {mx} {my}");
                                         let rs = sd.get(sid.as_str());
                                         if rs.is_some() {
                                             let mev = ManagedInput::event_from(&format!("{}{}","m",button));
                            //                        let mev = emgr.translate(ManagedInput::event_from(&format!("{}{}","m",button)));
                                             println!("Engine mev: {:?}",mev);
                                             let cs = rs.unwrap();
                                             match mev {
                                                 ManagedEvent::M1 => { cs.send_blocking(PlayerCommand::Skip_R).unwrap_or_default(); }
                                                 ManagedEvent::M2 =>{ cs.send_blocking(PlayerCommand::Skip_F).unwrap_or_default(); }
                                                 ManagedEvent::M3 => { cs.send_blocking(PlayerCommand::Reset).unwrap_or_default(); }
                                                 _ => {}
                                             }

                                             /*let bt = button.to_string();
                                             let cs = rs.unwrap().control_sender.clone();
                                             match bt.as_str() {
                                                 "left"=> { cs.send_blocking(PlayerCommand::Skip_R).unwrap_or_default(); }
                                                 "right"=>{ cs.send_blocking(PlayerCommand::Skip_F).unwrap_or_default(); }
                                                 _ => { cs.send_blocking(PlayerCommand::Reset).unwrap_or_default(); }
                                             }*/
                                         }
                                     }
                                     _ => {}
                                 }
                             });

                            let sd = senders.clone();
                            app.on_engine_exec(move |id,cmd| {
                                println!("Engine exec: {cmd}");
                                let sid = id.to_string();
                                let cas = cmd.to_string();;
                                match cas.as_str() {
                                    "clone" => {
                                        let rs = sd.get(sid.as_str());
                                        if rs.is_some() {
                                            let cs = rs.unwrap();
                                            cs.send_blocking(PlayerCommand::Clone_4).unwrap_or_default();
                                        }
                                    }
                                    "more" => {
                                        let rs = sd.get(sid.as_str());
                                        if rs.is_some() {
                                            let cs = rs.unwrap();
                                            cs.send_blocking(PlayerCommand::More).unwrap_or_default();
                                        }
                                    }
                                    "diff" => {
                                        let rs = sd.get(sid.as_str());
                                        if rs.is_some() {
                                            let cs = rs.unwrap();
                                            cs.send_blocking(PlayerCommand::Diff).unwrap_or_default();
                                        }
                                    }
                                    "quit" => std::process::exit(0),
                                    _ => {
                                        let itemi = usize::from_str_radix(&cas,10).unwrap_or_default();
                                        let nln = fd.items[itemi].0.as_str();
                                        println!("Switch to {itemi}->{nln}");
                                        unsafe {
                                            APP_CONSTS[MAIN_LIB] = itemi;
                                            match &nln[0..2] {
                                                "xx" => {
                                                    APP_CONSTS[QMODE] = 1;
                                                }
                                                _ => {
                                                    APP_CONSTS[QMODE] = 0;
                                                }
                                            }
                                        }
                                    }
                                }
                            });

                            let mut rids = &fids;
                            if unsafe { APP_CONSTS[QMODE] } !=0 {
                                rids = &qids;
                            }
                            for id in rids {
                                let p = &players[*id];
                                rc.render(p, wh)
                            }
                            app.window().request_redraw();
                        }
                    }
                    /*slint::RenderingState::BeforeRendering => {
                        if let (Some(underlay), Some(app)) = (underlay.as_mut(), app_weak.upgrade()) {
                            let is_paused = app.get_is_paused();
                            if prev_is_paused != is_paused {
                                if is_paused {
                                    underlay.pause();
                                } else {
                                    underlay.play();
                                }

                                prev_is_paused = is_paused;
                            }

                            if !app.get_position_ackd() {
                                app.set_position_ackd(true);
                                let duration = underlay.get_duration().unwrap_or(0) as f64;
                                let seek_target = app.get_new_position() as f64 / 100.0 * duration;
                                underlay.get_mpv().seek_absolute(seek_target);
                            }

                            app.set_ts_label(underlay.get_ts_label().into());

                            let position = underlay.get_position().unwrap_or(0) as f32;
                            let duration = underlay.get_duration().unwrap_or(0) as f32;

                            app.set_seek_position(position / duration);
                            underlay.render(wh);
                            app.window().request_redraw();
                        }
                    }
                    slint::RenderingState::AfterRendering => {}
                    slint::RenderingState::RenderingTeardown => {
                        drop(underlay.take());
                    }*/
                    _ => {}
                }




             //   });
            })
        {
            match error {
                slint::SetRenderingNotifierError::Unsupported => eprintln!("This example requires the use of the GL backend. Please run with the environment variable SLINT_BACKEND=GL set."),
                _ => unreachable!()
            }
            std::process::exit(1);
        }

        app.run().unwrap();
    });
}

