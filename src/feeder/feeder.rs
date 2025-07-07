use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct Feeder {
    order: String,
    pub(crate) items: Vec<(String, String)>,
    blacklists: Vec<(String,String)>,
    filter_min_size: u64
}

impl Feeder {
    pub fn new(cfg: &Vec::<(String,String)>) -> Self {
        let mut items = _filtered(cfg,"feeder.items.");
        let mut blacklists = _filtered(cfg,"feeder.filter.blacklist.");
        let order = _filtered(cfg,"feeder.order").first().unwrap().1.clone();
        let filter_min_size = u64::from_str_radix(_filtered(cfg,"feeder.filter.size.minMB").first().unwrap().1.as_str(),10).unwrap()*1024*1024;

        Self {
            order,
            filter_min_size,
            items,
            blacklists
        }
    }

    pub(crate) fn register(&self, tag: usize, itemi: usize) {
        unsafe {
            let pls = PlayerStatics::fetch(tag);
            pls.lib = itemi;
        }
    }

    fn filter(&self, itemi: usize, entries: Vec<String>) -> Vec<String> {
        let item = self.items[itemi].0.as_str();
        let mut ret = vec![];
        
        let mut blw = vec![];
        for b in self.blacklists.iter() {
            let mut mat = item.to_string();
            let k = b.0.clone();
            let w = k.find("*");
            if w.is_some() {
                let wi = w.unwrap();
                if mat.len() > wi {
                    mat = mat[..wi].to_string() + "*";
                } else {
                    continue;
                }
            } else {
                blw.extend(b.1.split('|'))
            }
            if k==mat {
                blw.extend(b.1.split('|'))
            }
        }
        
        for item in &entries {
            let f = fs::File::open(item).unwrap();
            let meta = f.metadata().unwrap();

            if meta.len() < self.filter_min_size {
         //       println!("Filtering: {:?}->{:?} < {}", f,meta,self.filter_min_size);
                continue;
            }

            let il = item.to_lowercase();
            let mut skip = false;
            for w in &blw {
                if il.contains(w) {
           //         println!("Filtering: {}->{}", w, il);
                    skip = true;
                    break;
                }
            }
            if skip { continue; }
            ret.push(item.clone());
        }
        println!("Filered {}->{}",entries.len(),ret.len());
//        thread::sleep(Duration::from_millis(1000));
        ret
    }

    pub fn clone_diff(&self,tag:usize)->String {
        let mut ret = String::new();
        unsafe {
            let pls = PlayerStatics::fetch(tag);
            let itemi = pls.lib;
            let mut path = &self.items[itemi].1.clone();
            let dirs = _dirs(path);
            loop {
                let mut idx_d = app_rand(dirs.len()+1);
                if dirs.len()>0 {
                    while idx_d == pls.show_idx { idx_d = app_rand(dirs.len()+1); }
                }
                let mut files =vec![];
                if idx_d == dirs.len() {
                    files = _files(path);
                } else {
                    files = _files(&dirs[idx_d]);
                }

                let filtered = self.filter(itemi, files);
                if filtered.len() == 0 { continue }

                let idx_f = app_rand(filtered.len());
                pls.show_idx = idx_d;
                pls.file_idx = idx_f;
                ret = filtered[idx_f].clone();
                break;
            }
        }
        ret
    }

    pub fn clone_more(&self,tag:usize)->String {
        let mut ret = String::new();
        unsafe {
            let pls = PlayerStatics::fetch(tag);
            let itemi = pls.lib;
            let path = &self.items[itemi].1;
            let dirs = _dirs(path);
            loop {
                let mut files = vec![];
                let idx_d = pls.show_idx;;
                if idx_d == dirs.len() {
                    files = _files(path);
                } else {
                    files = _files(&dirs[idx_d]);
                }
                let filtered = self.filter(itemi, files);
                if filtered.len()==0 { continue }

                let idx_f = app_rand(filtered.len());
                ret = filtered[idx_f].clone();
                break;
            }
        }
        ret
    }
    
    pub fn clone_tagged(&self,src:usize,tag:usize)->String {
        let mut ret = String::new();
        unsafe {
            let pls = PlayerStatics::fetch(tag);
            let other = PlayerStatics::fetch(src);
            let idx_do = tag * 3;
            let itemi = other.lib;
            let path = &self.items[itemi].1;
            let idx_d = other.show_idx;
            let idx_f = other.file_idx;
            let dirs = _dirs(path);
            loop {
                let mut files = vec![];
                if idx_d == dirs.len() {
                    files = _files(path);
                } else {
                    files = _files(&dirs[idx_d]);
                }
                let filtered = self.filter(itemi, files);
                if filtered.len()==0 { continue }

                pls.lib = other.lib;
                pls.show_idx = other.show_idx;
                pls.file_idx = other.file_idx;
                ret = filtered[idx_f].clone();
                break;
            }
        }
        ret
    }
    
    pub fn next_tagged(&self, tag: usize) -> String {
        let mut ret = String::new();
        unsafe {
            let pls = PlayerStatics::fetch(tag);
            let itemi = pls.lib;
            let path = &self.items[itemi].1;
            let dirs = _dirs(path);
            loop {
                let idx_d = app_rand(dirs.len() + 1);
                let mut files = vec![];
                if idx_d == dirs.len() {
                    files = _files(path);
                } else {
                    files = _files(&dirs[idx_d]);
                }

                let filtered = self.filter(itemi, files);
                if filtered.len() == 0 { continue; }

                let idx_f = app_rand(filtered.len());
                /*let mut skip = false;
                if tag > 0 && tag < 4 {
                    for t in 1..4 {
                        if OUTPUT_CONSTS[t * 3] == idx_d && OUTPUT_CONSTS[t * 3 + 1] == idx_f {
                            skip = true;
                        }
                    }
                }*/
                //if skip { continue; }

                pls.show_idx = idx_d;
                pls.file_idx = idx_f;

                ret = filtered[idx_f].clone();
                break;
            }
        }
        ret
    }
}