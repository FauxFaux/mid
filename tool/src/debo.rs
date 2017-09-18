use std::env;
use std::fs;
use std::io;

use std::collections::HashMap;
use std::collections::HashSet;

use errors::*;

use casync_format;
use casync_http::Chunk;
use casync_http::ChunkId;
use git2::Repository;
use reqwest;

pub fn debo(pkg: &str, config: &::Config) -> Result<()> {
    let client = reqwest::Client::new()?;

    let dest = {
        let mut dest = env::current_dir()
            .chain_err(|| "determining current directory")?
            .to_path_buf();

        dest.push(pkg);
        if dest.exists() {
            bail!("checkout directory already exists: {:?}", dest);
        }

        dest
    };

    let sources = ::lists::sources(&client, config).chain_err(
        || "fetching sources list",
    )?;

    // TODO: strsim matching
    // TODO: check binary package names
    let versions = sources.get(pkg).ok_or("package not found")?;

    let repacked_fetcher = {
        let mut local_cache = config.cache_root.clone();
        local_cache.push("repacked.castr");

        fs::create_dir_all(&local_cache).chain_err(|| {
            format!("creating cache directory: {:?}", local_cache)
        })?;

        ::casync_http::Fetcher::new(
            &client,
            &config.casync_mirror,
            local_cache,
            "data/origs/default.castr",
        ).chain_err(|| "validating fetcher settings")?
    };

    let mut version_chunks: HashMap<String, Vec<Chunk>> = HashMap::with_capacity(versions.len());
    let mut all_required_chunks: HashSet<ChunkId> = HashSet::new();

    for version in versions {
        let chunks = repacked_fetcher
            .parse_whole_index(format!(
                "data/origs/{}/{}/{}.caidx",
                prefix_of(pkg),
                pkg,
                version
            ))
            .chain_err(|| {
                format!("loading index for package {} version {}", pkg, version)
            })?;

        all_required_chunks.reserve(chunks.len());
        for chunk in &chunks {
            all_required_chunks.insert(chunk.id);
        }

        version_chunks.insert(version.to_string(), chunks);
    }

    repacked_fetcher
        .fetch_all_chunks(all_required_chunks.iter())
        .chain_err(|| "fetching raw data for package")?;

    let repo: Repository = Repository::init(&dest).chain_err(|| {
        format!("creating repository at {:?}", dest)
    })?;

    for version in versions {
        let mut chunks = version_chunks.get(version).unwrap().into_iter();

        let reader = casync_format::ChunkReader::new(|| {
            Ok(match chunks.next() {
                Some(chunk) => Some(chunk.open_from(repacked_fetcher.local_store())?),
                None => None,
            })
        }).chain_err(|| "initialising reader")?;

        let mut tree = repo.treebuilder(None)?;

        casync_format::read_stream(reader, |path, entry, data| {
            if entry.is_dir() {
                // just totally ignoring directories
                return Ok(());
            }

            let raw_path: Vec<u8> = path.join(&b'/');

            println!("{:?} {:?} {:?}", path, raw_path.clone(), String::from_utf8(raw_path.clone()));

            let oid = {
                let mut writer = repo.blob_writer(None).map_err(|e| format!("TODO git error: {}", e))?;

                // TODO: symlinks
                io::copy(&mut data.ok_or("expecting data for a non-directory")?, &mut writer)?;

                writer.commit().map_err(|e| format!("TODO git error: {}", e))?
            };

            tree.insert(
                raw_path,
                oid,
                // TODO: executable, symlink
                0o100644,
            ).map_err(|e| format!("TODO git error: {}", e))?;

            Ok(())
        }).chain_err(|| "reading stream")?;

        let oid = tree.write()?;
    }

    unimplemented!()
}


fn prefix_of(pkg: &str) -> &str {
    if pkg.starts_with("lib") && pkg.len() > 3 {
        &pkg[0..4]
    } else {
        &pkg[0..1]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn prefix() {
        use super::prefix_of;
        assert_eq!("b", prefix_of("bash"));
        assert_eq!("libb", prefix_of("libbadger"));

        // Who knows what this should do; no examples currently.
        assert_eq!("b", prefix_of("b"));
        assert_eq!("liba", prefix_of("liba"));
    }
}
