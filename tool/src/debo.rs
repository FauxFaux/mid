use std::fs;

use std::collections::HashMap;
use std::collections::HashSet;

use errors::*;

use casync_http::Chunk;
use casync_http::ChunkId;
use reqwest;

pub fn debo(pkg: &str, config: &::Config) -> Result<()> {
    let client = reqwest::Client::new()?;

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

    unimplemented!()
}


fn prefix_of(pkg: &str) -> &str {
    if pkg.starts_with("lib") && pkg.len() > 3 {
        &pkg[0..4]
    } else {
        &pkg[0..1]
    }
}
