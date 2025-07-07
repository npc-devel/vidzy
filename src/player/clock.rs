use ffmpeg_the_third::format::stream;
use std::time;
use std::time::Duration;
use chrono::prelude::*;

struct StreamClock {
    time_factor: f64,
    time_base_seconds: f64,
    //start_time: std::time::Instant,
    start_ms: f64
}

impl StreamClock {
    const MAX_DELAY:f64 = 60.0;
    const MIN_DELAY:f64 = 0.0;
    
    fn now(factor: f64) -> f64 {
        //let n = std::time::Instant::now();
        //let s = n.as_millis() as f64*factor;
        //std::time::Instant::now()
        let now = Utc::now();
        let ret = now.timestamp_millis() as f64;//s f64 * factor;
    //    println!("Now: {factor}");
        ret
    }
    
    fn new(stream: &stream::Stream,time_factor: f64) -> Self {
        let time_base_seconds = stream.time_base();
        let time_base_seconds =
            time_base_seconds.numerator() as f64 / time_base_seconds.denominator() as f64;
        let start_ms = Self::now(time_factor);
        
        Self { time_factor, time_base_seconds, start_ms }
    }
    
    fn hit_end(&self, pts: Option<i64>,end_secs: f64)->bool {
        if pts.is_some() && end_secs > 0.0  {
            let ap = pts.unwrap();
            let pts_since_start = 1000.0*ap as f64 * self.time_base_seconds;
            1000.0*end_secs <= pts_since_start
        } else {
            false
        }
    }

    fn pts_to_secs(&self, pts: Option<i64>)->f64 {
        //if pts.is_some()  {
            let ap = pts.unwrap();
            //-let pts_since_start = 
                ap as f64 * self.time_base_seconds * self.time_factor
          //  1000.0*end_secs <= pts_since_start
        //} else {
          //  false
        //}
    }
    
    fn convert_pts_to_instant(&mut self, pts: Option<i64>,start_secs: f64) -> Option<std::time::Duration> {
        if pts.is_some() {
          //  if self.start_ms == 0.0 {
        //        self.start_ms = Self::now(self.time_factor);
      //      }
            let ap = pts.unwrap(); 
            let pts_since_start = self.time_factor * (1000.0*ap as f64 * self.time_base_seconds - 1000.0*start_secs);
            let ms_since_start = Self::now(self.time_factor) - self.start_ms;// - 1000.0*start_secs);
            let mut delay = 0.0;
            if pts_since_start > ms_since_start {
                delay = pts_since_start - ms_since_start;
            }
           // println!("Frame delay: {} {} -> {}",pts_since_start,ms_since_start,delay);
            //if delay > Self::MAX_DELAY { delay = 30.0; }
            //else if delay < Self::MIN_DELAY { delay = Self::MIN_DELAY; }
            
            //if delay == 0.0 {
            //    self.start_ms == Self::now(self.time_factor) - pts_since_start/self.time_factor;
            //}
            
            Some(Duration::from_millis(delay as u64))
        } else {
            Option::None
        }
        /*pts.and_then(|pts| {
            //let secs_since_start = Duration::from_secs_f64(pts as f64 * self.time_base_seconds);
            //self.start_time.checked_add(secs_since_start).unwrap().checked_sub(Duration::from_secs_f64(start_secs))
            let ms_since_start = 1000.0 * pts as f64 * self.time_base_seconds;
            (self.start_ms + ms_since_start - 1000.0*start_secs)
        })
            .map(|absolute_pts| absolute_pts.duration_since(std::time::Instant::now()))*/
    }
}