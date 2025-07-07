pub fn _dirs(path: &str)-> Vec::<String> {
  //  println!("Dirs: {:?}",path);
    let mut ret = vec![];
    let ir = path.find('{');
    if ir.is_some() {
        let i = ir.unwrap();
        let root = &path[..i];
        let rest = path[i+1..path.len()-1].split('|').to_owned().into_iter();
        for r in rest {
            println!("{}/{}",root,r);
            ret.push(format!("{}/{}",root,r))
        }
    } else {
        let dir = fs::read_dir(path);
        if dir.is_ok() {
            let dir = dir.unwrap();
            for entry in dir {
                let entry = entry.unwrap();
                let ep = entry.path();
                if ep.is_dir() {
                    ret.push(ep.to_str().unwrap().to_string());
                }
            }
        }
    }
    ret.sort();
    ret
}


pub fn _files(path: &str)-> Vec::<String> {
//    println!("Files: {:?}",path);
    let mut ret = vec![];
    let dir = fs::read_dir(path);
    if dir.is_ok() {
        let dir = dir.unwrap();
        for entry in dir {
            let entry = entry.unwrap();
            let ep = entry.path();
            if ep.is_dir() {
                ret.extend(_files(ep.as_path().to_str().unwrap()));
            } else {
                ret.push(ep.to_str().unwrap().to_string());
            }
        }
    }
    ret.sort();
    ret
}

pub fn _conf(id: &str)-> Vec::<(String, String)> {
    let mut ret = vec![];
    let res = fs::read_to_string(format!("{}/{}.conf", ROOT_PATH, id));
    if res.is_ok() {
        let raw = res.unwrap();
        let lines = raw.split('\n');
        for line in lines {
            let mut s = line.split('#');
            let g = s.nth(0).unwrap_or("");
            if g.len() > 0 {
                let i = g.find("=").unwrap_or(0);
                if i > 0 {
                    let nme = &g[0..i].trim();
                    let val = &g[i+1..].trim();
                    ret.push((nme.to_string(), val.to_string()));
                }
            }
        }
    }
    ret
}

pub fn _filtered(src: &Vec::<(String,String)>,mat: &str)->Vec::<(String,String)> {
    let mut ret = vec![];
    for k in src {
        let f = k.0.find(mat);
        if f.is_some() {
            let mut item = &k.0[mat.len()..];
            if item.len() == 0 {
                item = k.0.as_str();
            }
            ret.push((item.to_string(), k.1.to_owned()));
        }
    }
  //  println!("Filtered: {:?}",ret.clone());
    ret
}

pub fn _single(src: &Vec::<(String,String)>,mat: &str)->String {
    _filtered(src,mat).first().unwrap().1.to_owned()
}