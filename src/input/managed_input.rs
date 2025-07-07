use std::cmp::PartialEq;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::ManuallyDrop;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::helpers::{_filtered, _single};

#[derive(Clone, Copy, Debug)]
struct KeyFrame {
    event_code: ManagedEvent,
    ev_time: i64,
    action_code: ManagedEvent
}

#[derive(Clone, Copy,Debug,PartialEq)]
pub enum ManagedEvent {
    N,
    M1,
    M2,
    M3
}

pub struct ManagedInput {
    key_frames: Vec<KeyFrame>,
    init_time: SystemTime,
    combo_frames: HashMap<String, Vec<KeyFrame>>,
    unit_interval: i64
}
impl ManagedInput {
    pub fn new(cfg: &Vec::<(String,String)>) -> Self {
        let mut combo_frames = HashMap::<String,Vec<KeyFrame>>::new();
        let unit_interval = i64::from_str_radix(_single(cfg,"input.combos.unit-interval").as_str(),10).unwrap();
        let filtered = _filtered(cfg,"input.combos.pat.");

        for i in filtered.iter() {
            let mut s= i.0.split('.');
            let name = s.nth(0).unwrap().to_string();
            let value = s.nth(0).unwrap().to_string();
            if !combo_frames.contains_key(&name) {
                combo_frames.insert(name.clone(),vec![]);
            }
            match value.as_str() {
                "combo" => {
                    let f = combo_frames.get_mut(&name).unwrap();
                    let fd = i.1.split('|');
                    let mut it = -1*unit_interval*fd.clone().count() as i64;
                    for k in fd {
                        f.push(KeyFrame { event_code: Self::event_from(k), ev_time: it, action_code: ManagedEvent::N });
                        it += unit_interval;
                    }
                },
                _=> {}
            }
        }

     //   for i in combo_frames.iter_mut() {
       //     let f = filtered.get(&format!("{}.action",i.0.clone())).unwrap();
         //   i.1.last_mut().unwrap().action_code = Self::event_from(f.as_str());
        //}
        //println!("Combo frames: {:?}",combo_frames);

        Self {
            unit_interval,
            key_frames: vec![],
            init_time: SystemTime::now(),
            combo_frames
        }
    }

    pub fn ev_time(&self)->i64 {
        SystemTime::now().duration_since(self.init_time).unwrap().as_millis() as i64
    }

    pub(crate) fn event_from(str: &str) ->ManagedEvent {
        match str {
            "m1"|"mleft" => ManagedEvent::M1,
            "m2"|"mright" => ManagedEvent::M2,
            "m3"|"mmiddle" => ManagedEvent::M3,
            _ => ManagedEvent::N
        }
    }

    fn resolve(&mut self)->ManagedEvent {
        let rev = self.key_frames.iter().rev();;
        let enow = self.ev_time();
        let mut nmatched: HashMap<&String,ManagedEvent> = HashMap::new();
        let mut matched: HashMap<&String,ManagedEvent> = HashMap::new();

        for i in self.combo_frames.keys() {
            nmatched.insert(i, ManagedEvent::N);
        }
       // let mut mt = enow - rev.len() as i64 * self.unit_interval;

        
            for mk in nmatched.clone().keys() {
                for kf in rev.clone() {
                    let mt = kf.ev_time;
                    let rf = self.combo_frames[*mk].iter().clone().rev();
                    for f in rf {
                        let dt = (enow + f.ev_time - mt).abs() * 2;
                        println!("Check: {dt} {}/{}", f.ev_time, mt);
                        if f.event_code == kf.event_code && dt < self.unit_interval {
                            println!("Matching: {mk}");
                            nmatched.remove(mk);
                            matched.insert(mk, (*f).clone().action_code);
                        }
                    }
                }
            }
            for mk in matched.clone().keys() {
                for kf in rev.clone() {
                    let mt = kf.ev_time;
                let rf = self.combo_frames[*mk].iter().clone();
                let dtex = (enow + self.combo_frames[*mk].last().unwrap().ev_time - mt).abs()*2;
                println!("ULCheck: {dtex}");
                if dtex > self.unit_interval {
                    println!("TUnmatching: {mk}");
                    matched.remove(mk);
                    nmatched.insert(mk,ManagedEvent::N);
                } else {
                    for f in rf {
                        let dt = (enow + f.ev_time - mt).abs() * 2;
                        println!("UCheck: {dt} {:?}", f);

                        if dt < self.unit_interval && f.event_code != kf.event_code {
                            println!("CUnmatching: {mk}");
                            matched.remove(mk);
                            nmatched.insert(mk, ManagedEvent::N);
                        }
                    }
                }
            }
            //mt += self.unit_interval;
        }
        println!("Matched----> {:?}",matched);
        if matched.len() == 1 {
            let cf = matched.keys().next().unwrap();
            self.combo_frames.get(*cf).unwrap().clone().last().unwrap().clone().action_code
        } else if matched.len() > 0 {
            ManagedEvent::N
        } else {
            self.key_frames.last().unwrap().clone().event_code
        }
    }

    pub fn translate(&mut self,event_code: ManagedEvent)->ManagedEvent {
        println!("Translating: {:?}",event_code);
        self.key_frames.push(KeyFrame {event_code,ev_time: self.ev_time(),action_code: ManagedEvent::N});
        self.resolve()
    }
}