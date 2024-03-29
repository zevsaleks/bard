use std::slice;

use globset::Glob;
use serde::Deserialize;

use crate::prelude::*;
use crate::util::{read_dir_all, sort_paths_lexical};

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum SongsGlobs {
    One(String),
    Many(Vec<String>),
}

impl SongsGlobs {
    pub fn iter(&self) -> impl Iterator<Item = &str> + '_ {
        let items = match self {
            Self::One(one) => slice::from_ref(one),
            Self::Many(many) => many.as_slice(),
        };

        (0..).map_while(move |i| items.get(i).map(move |s| s.as_str()))
    }
}

impl Default for SongsGlobs {
    fn default() -> Self {
        Self::One("*.md".into())
    }
}

#[derive(Debug)]
pub struct InputSet<'a> {
    dir_songs: &'a Path,
    all_files: Vec<PathBuf>,
    match_set: Vec<PathBuf>,
}

impl<'a> InputSet<'a> {
    pub fn new(dir_songs: &'a Path) -> Result<Self> {
        let all_files = read_dir_all(dir_songs)
            .with_context(|| format!("Could not read directory {:?}", dir_songs))?;

        Ok(Self {
            dir_songs,
            all_files,
            match_set: vec![],
        })
    }

    fn is_globlike<S: AsRef<str>>(s: S) -> bool {
        s.as_ref().contains(&['*', '?', '{', '}'][..])
    }

    fn apply_glob_inner<'s>(&'s mut self, glob: &str) -> Result<&'s mut [PathBuf]> {
        let orig_len = self.match_set.len();
        let glob = Glob::new(glob)
            .with_context(|| format!("Invalid glob pattern: '{}'", glob))?
            .compile_matcher();
        let dir_songs = self.dir_songs;
        let match_set = &mut self.match_set;

        for matched in self
            .all_files
            .iter()
            // NB. Unwrap should be ok here as the paths will all be prefixed by dir_songs
            .filter(|path| glob.is_match(path.strip_prefix(dir_songs).unwrap()))
        {
            match_set.push(matched.clone());
        }

        Ok(&mut match_set[orig_len..])
    }

    pub fn apply_glob(mut self, glob: &str) -> Result<Self> {
        if Self::is_globlike(glob) {
            // This might be a glob
            let added = self.apply_glob_inner(glob)?;
            if added.is_empty() {
                bail!(
                    "No files matched pattern '{}' in diectory {:?}",
                    glob,
                    self.dir_songs,
                );
            }

            // Sort the entries collected for this glob.
            // This way, paths from one glob pattern are sorted alphabetically,
            // but order of globs as given in the input array is preserved.
            sort_paths_lexical(added);
        } else {
            // This is a plain filename
            let path = self.dir_songs.join(glob);
            if !path.exists() {
                bail!("File not found: {:?}", path);
            }

            self.match_set.push(path);
        }

        Ok(self)
    }

    pub fn finalize(self) -> Result<Vec<PathBuf>> {
        Ok(self.match_set)
    }
}
