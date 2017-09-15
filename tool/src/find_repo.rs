use std::path::Path;
use std::path::PathBuf;

use errors::*;

pub struct TempRepo {
    pub root: PathBuf,
    pub mid: bool,
}

pub fn find_repos<P: AsRef<Path>>(from: P) -> Result<Vec<TempRepo>> {
    let mut now = from.as_ref().to_path_buf();
    let mut ret = Vec::with_capacity(2);

    loop {
        let mut cand = now.clone();

        if !now.pop() {
            break;
        }

        cand.push(".git");
        cand.push("HEAD");

        if !cand.is_file() {
            // not a git repo
            continue;
        }

        assert!(cand.pop());
        cand.push("mid");

        let mid = cand.is_dir();

        assert!(cand.pop());
        assert!(cand.pop());

        ret.push(TempRepo { root: cand, mid });
    }

    Ok(ret)
}
