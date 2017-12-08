use std::fs;
use std::io;
use std::path::Path;
use std::time;

use std::collections::HashMap;
use std::io::BufRead;

use reqwest::Client;
use zstd::Decoder;

use errors::*;

pub fn sources(client: &Client, config: &::Config) -> Result<HashMap<String, Vec<String>>> {
    let mut path = config.cache_root.clone();
    path.push("lists");
    fs::create_dir_all(&path).chain_err(|| format!("creating lists directory at {:?}", path))?;

    path.push("sources.zstd");

    if outdated(&path).chain_err(|| {
        format!("checking if sources list ({:?}) is outdated", path)
    })? {
        let url = format!("{}/data/sources.zstd", config.casync_mirror);

        let mut resp = client
            .get(&url)
            .send()
            .chain_err(|| format!("downloading {}", url))?;

        if !resp.status().is_success() {
            bail!("downloading {} failed: {}", url, resp.status());
        }

        let mut temp = ::tempfile_fast::persistable_tempfile_in(path.parent().unwrap())
            .chain_err(|| "creating download temporary file")?;

        // TODO: length checks?
        io::copy(&mut resp, temp.as_mut())
            .chain_err(|| "writing downloaded sources to temporary file")?;

        if let Err(e) = fs::remove_file(&path) {
            // TODO: log ignored warning?
        }

        temp.persist_noclobber(&path)
            .chain_err(|| format!("saving downloaded sources to {:?}", path))?;
    }

    let mut sources: HashMap<String, Vec<String>> = HashMap::with_capacity(30_000);

    let file = io::BufReader::new(Decoder::new(fs::File::open(path)?)?);

    for line in file.lines() {
        let line = line?;
        let mut bits = line.split(' ');
        let pkg = bits.next()
            .ok_or("invalid blank line in sources list")?
            .to_string();
        let mut all_versions: Vec<String> = bits.map(|x| x.to_string()).collect();
        all_versions.sort_by(|left, right| ::deb_version::compare_versions(left, right));
        sources.insert(pkg, all_versions);
    }

    Ok(sources)
}

fn outdated<P: AsRef<Path>>(path: P) -> Result<bool> {
    let four_hours = time::Duration::new(4 * 60 * 60, 0);

    match path.as_ref().metadata() {
        Ok(meta) => {
            match meta.modified() {
                Ok(mtime) => {
                    match time::SystemTime::now().duration_since(mtime) {
                        Ok(difference) => Ok(difference > four_hours),
                        Err(_) => Ok(true), // file from the future
                    }
                }
                Err(_) => Ok(true), // unsupported platform
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) => bail!(e),
    }
}
