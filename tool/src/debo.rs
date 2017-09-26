use std::env;
use std::fs;
use std::io;
use std::path;

use std::collections::HashMap;
use std::collections::HashSet;

use errors::*;

use casync_format;
use casync_http::Chunk;
use casync_http::ChunkId;
use git2;
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
    let versions: &Vec<String> = sources.get(pkg).ok_or("package not found")?;

    let repacked_chunks = download(&client, config, "repacked", pkg, &versions)?;
    let debdir_chunks = download(&client, config, "debdir", pkg, &versions)?;

    let repo: Repository = Repository::init(&dest).chain_err(|| {
        format!("creating repository at {:?}", dest)
    })?;

    let mid_signature = git2::Signature::new("mid", "mid@goeswhere.com", &git2::Time::new(0, 0))?;
    let mut previous_repacked_commit = None;
    let mut previous_debdir_commit = None;

    for version in versions {

        let repacked_tree = chunks_to_tree(
            &repo,
            &repacked_chunks.downloaded_into,
            repacked_chunks.by_version.get(version).unwrap().into_iter(),
        )?;

        let repacked_commit = repo.find_commit(repo.commit(
            Some("refs/heads/repacked"),
            &mid_signature,
            &mid_signature,
            &format!("Repacked {}:{}", pkg, version),
            &repacked_tree,
            &previous_repacked_commit
                .iter()
                .collect::<Vec<&git2::Commit>>(),
        )?)?;

        let debian_tree = chunks_to_tree(
            &repo,
            &debdir_chunks.downloaded_into,
            debdir_chunks.by_version.get(version).unwrap().into_iter(),
        )?;

        let mut with_deb = repo.treebuilder(Some(&repacked_tree))?;
        with_deb.insert("debian", debian_tree.id(), 0o040000)?;
        let with_deb = repo.find_tree(with_deb.write()?)?;

        let debdir_commit = {
            let mut parents: Vec<&git2::Commit> = previous_debdir_commit
                .iter()
                .collect::<Vec<&git2::Commit>>();
            parents.push(&repacked_commit);

            repo.find_commit(repo.commit(
                Some(&format!("refs/heads/skip-patches-{}", version)),
                &mid_signature,
                &mid_signature,
                &format!("Adding /debian/ from {}:{}", pkg, version),
                &with_deb,
                &parents,
            )?)
        }?;

        previous_debdir_commit = Some(debdir_commit);
        previous_repacked_commit = Some(repacked_commit);
    }

    Ok(())
}

struct DownloadedChunks {
    downloaded_into: path::PathBuf,
    by_version: HashMap<String, Vec<Chunk>>,
}

fn download(
    client: &reqwest::Client,
    config: &::Config,
    mirror_type: &str,
    pkg: &str,
    versions: &[String],
) -> Result<DownloadedChunks> {
    let fetcher = {
        let mut local_cache = config.cache_root.clone();
        local_cache.push(format!("{}.castr", mirror_type));

        fs::create_dir_all(&local_cache).chain_err(|| {
            format!("creating cache directory: {:?}", local_cache)
        })?;

        ::casync_http::Fetcher::new(
            &client,
            &config.casync_mirror,
            local_cache,
            &format!("data/{}/default.castr", mirror_type),
        ).chain_err(|| "validating fetcher settings")?
    };

    let mut version_chunks: HashMap<String, Vec<Chunk>> = HashMap::with_capacity(versions.len());
    let mut all_required_chunks: HashSet<ChunkId> = HashSet::new();

    for version in versions {
        let chunks = fetcher
            .parse_whole_index(format!(
                "data/{}/{}/{}/{}.caidx",
                mirror_type,
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

    fetcher
        .fetch_all_chunks(all_required_chunks.iter())
        .chain_err(|| "fetching raw data for package")?;

    Ok(DownloadedChunks {
        downloaded_into: fetcher.local_store(),
        by_version: version_chunks,
    })
}

fn chunks_to_tree<'i, I, P: AsRef<path::Path>>(
    repo: &Repository,
    local_store: P,
    mut chunks: I,
) -> Result<git2::Tree>
where
    I: Iterator<Item = &'i Chunk>,
{
    let local_store = local_store.as_ref();

    let reader = casync_format::ChunkReader::new(|| {
        Ok(match chunks.next() {
            Some(chunk) => Some(chunk.open_from(local_store)?),
            None => None,
        })
    }).chain_err(|| "initialising reader")?;

    let mut tree: HashMap<Vec<u8>, GitNode> = HashMap::new();

    casync_format::read_stream(reader, |path, entry, data| {
        if entry.is_dir() {
            // just totally ignoring directories
            return Ok(());
        }

        let oid = {
            // TODO: the blob_writer api isn't ideal here; libgit2-c suggests using
            // TODO: `git_odb_open_wstream` if you know the size, which currently isn't exposed.
            // TODO: I suspect that we can get a speedup by reading up to a megabyte(?) into
            // TODO: memory, and dumping that all out through the `repo.blob` api.
            // TODO: Pull request here: https://github.com/alexcrichton/git2-rs/issues/238

            let mut writer = repo.blob_writer(None).map_err(|e| {
                format!(
                    concat!(
                        "git couldn't prepare to write a blob",
                        " (TODO: extra error information lost): {}"
                    ),
                    e
                )
            })?;

            // TODO: symlinks
            io::copy(
                &mut data.ok_or("expecting data for a non-directory")?,
                &mut writer,
            )?;

            writer.commit().map_err(|e| {
                format!(
                    concat!(
                        "git couldn't write the blob out",
                        " (TODO: extra error information lost): {}"
                    ),
                    e
                )
            })?
        };

        write_map(&mut tree, path, oid, 0o100644);

        Ok(())
    }).chain_err(|| "reading stream")?;

    Ok(repo.find_tree(write_tree(&repo, tree)?)?)
}

fn write_map(
    mut into: &mut HashMap<Vec<u8>, GitNode>,
    path: &[Vec<u8>],
    oid: git2::Oid,
    mode: i32,
) {
    use std::collections::hash_map::Entry::*;

    match path.len() {
        0 => unreachable!(),
        1 => {
            into.insert(path[0].clone(), GitNode::File { oid, mode });
        }
        _ => {
            match into.entry(path[0].clone()) {
                Occupied(mut exists) => {
                    match exists.get_mut() {
                        &mut GitNode::Dir(ref mut map) => write_map(map, &path[1..], oid, mode),
                        _ => panic!("TODO: invalid directory stream"),
                    }
                }
                Vacant(vacancy) => {
                    let mut map = HashMap::new();
                    write_map(&mut map, &path[1..], oid, mode);
                    vacancy.insert(GitNode::Dir(map));
                }
            }
        }
    }
}

fn write_tree(repo: &Repository, from: HashMap<Vec<u8>, GitNode>) -> Result<git2::Oid> {
    let mut builder = repo.treebuilder(None)?;
    for (path, thing) in from {
        let (oid, mode) = match thing {
            GitNode::File { oid, mode } => (oid, mode),
            GitNode::Dir(entries) => (write_tree(repo, entries)?, 0o040000),
        };

        builder.insert(path, oid, mode)?;
    }

    Ok(builder.write()?)
}

enum GitNode {
    Dir(HashMap<Vec<u8>, GitNode>),
    File { oid: git2::Oid, mode: i32 },
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
