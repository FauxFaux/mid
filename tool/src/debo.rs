use errors::*;

use reqwest;

pub fn debo(pkg: &str, config: &::Config) -> Result<()> {
    let client = reqwest::Client::new()?;

    let sources = ::lists::sources(&client, config)?;
    let versions = sources.get(pkg).ok_or("package not found")?;

    println!("{:?}", versions);

    let repacked_fetcher = {
        let mut local_cache = config.cache_root.clone();
        local_cache.push("repacked.castr");

        ::casync_http::Fetcher::new(
            &client,
            &config.casync_mirror,
            local_cache,
            "orig/default.castr",
        )?
    };



    unimplemented!()
}
