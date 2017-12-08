use std;
use std::collections::HashMap;

use errors::*;

const INDEX_TRAILER: &[u8] = b"===================================================================";

struct Hunk {
    spec: Vec<u8>,
    lines: Vec<Vec<u8>>,
}

pub struct Patch {
    header: Vec<u8>,
    file_changes: HashMap<String, Vec<Hunk>>,
    have_index_markers: bool,
}

pub fn parse(blob: &[u8]) -> Result<Patch> {
    let lines: Vec<&[u8]> = blob.split(|c| b'\n' == *c).collect();
    let mut lines = lines.into_iter().peekable();

    let mut header = vec![0u8; 0];
    loop {
        match lines.peek() {
            Some(line) if line.starts_with(b"--- ") || line.starts_with(b"Index: ") => {
                break;
            }
            Some(line) => {
                header.extend(*line);
                header.push(b'\n');
            }
            None => bail!("couldn't find any patch in a patch file"),
        }
        lines.next().unwrap();
    }

    let mut have_index_markers = false;
    let mut file_changes = HashMap::new();

    loop {
        let mut line = match lines.next() {
            Some(line) => line,
            None => break,
        };

        if line.starts_with(b"Index: ") {
            have_index_markers = true;

            let trailer = lines.next().ok_or("a line must follow an index line")?;
            ensure!(
                trailer == INDEX_TRAILER,
                "line following index line should be full of equals: {:?}",
                String::from_utf8(trailer.to_vec())
            );

            line = lines.next().ok_or(
                "a patch line must follow an index block",
            )?;
        }

        ensure!(line.starts_with(b"--- "), "a removal path is missing");
        let removal_path = &line[4..];

        let line = lines.next().ok_or("no line for addition path")?;
        ensure!(line.starts_with(b"+++ "), "addition path is invalid");
        let addition_path = &line[4..];

        ensure!(
            removal_path.starts_with(b"a/") && addition_path.starts_with(b"b/"),
            "paths aren't prefixed"
        );

        ensure!(
            removal_path[2..] == addition_path[2..],
            "paths aren't equal"
        );
        let path = String::from_utf8(addition_path[2..].to_vec())?;

        let mut hunks = Vec::new();

        loop {
            let line = match lines.next() {
                Some(line) if !line.is_empty() => line,
                _ => break,
            };

            ensure!(line.starts_with(b"@@ "), "spec line is invalid");
            let spec = &line[3..];

            let mut hunk_lines = Vec::new();

            loop {
                match lines.peek() {
                    Some(line) if line.is_empty() || b'@' == line[0] => break,
                    Some(line) => hunk_lines.push(line.to_vec()),
                    None => break,
                }
                lines.next().unwrap();
            }

            hunks.push(Hunk {
                spec: spec.to_vec(),
                lines: hunk_lines,
            });
        }

        file_changes.insert(path, hunks);
    }

    Ok(Patch {
        header,
        file_changes,
        have_index_markers,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_parse() {
        parse(&include_bytes!("../tests/data/bash43-004.diff")[..]).unwrap();
    }
}
