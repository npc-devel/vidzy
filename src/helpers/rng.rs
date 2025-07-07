use rand::Rng;
use slint::private_unstable_api::re_exports::euclid::num::Round;


#[link(name = "c")]
unsafe extern "C" {
    fn rand() -> i32;
    fn srand(seed: u32);
}

pub fn nanos() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
}

fn c_seed() {
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    unsafe {
        srand(seed);
    }
}

fn c_rand(limit: usize) -> usize {
    let mut r: usize = 0;
    unsafe {
        r = rand() as usize % limit;
    }
    r
}

static mut wh_seeds: [u16;3] = [0,0,0];
fn rng_rand(limit: usize) -> usize {
    let mut rng = rand::rng();
    rng.random_range(0..limit)
}

fn wh_rand_raw(limit: usize) -> usize {
    let mut ret = 0usize;
    let nl = limit + 1;
    loop {
        unsafe {
            wh_seeds[0] = ((171 * wh_seeds[0] as u64) % 30269) as u16;
            wh_seeds[1] = ((172 * wh_seeds[1] as u64) % 30307) as u16;
            wh_seeds[2] = ((170 * wh_seeds[2] as u64) % 30323) as u16;
            let fac = wh_seeds[0] as f64 / 30269.0f64 *
                           wh_seeds[1] as f64 / 30307.0f64 *
                           wh_seeds[2] as f64 / 30323.0f64;
            let float = fac - fac.floor();
            ret = (float * nl as f64) as usize;
        }
        if ret != 0 { break }
    }
    ret -= 1;
 //   println!("Rand={ret}/{limit}");
    ret
}

fn wh_rand(limit: usize) -> usize {
    if limit < 2 { return 0; }
    
    let lv = wh_rand_raw(limit);
    let hv = limit - wh_rand_raw(limit) - 1;
    ((lv+hv)/2).round() as usize
}

pub fn app_seed() {
    unsafe {
        wh_seeds = [
            nanos() as u16,
            nanos() as u16,
            nanos() as u16
        ];
        // println!("{} {} {}",wh_seeds[0],wh_seeds[1],wh_seeds[2]);
    }
}

pub fn app_rand(limit: usize)->usize {
    if limit < 2 { return 0; }
    
    rng_rand(limit)
}