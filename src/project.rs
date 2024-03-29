use std::collections::BTreeMap;
use std::fs;
use std::iter;
use std::process::Command;
use std::process::Stdio;
use std::str;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::app::App;
use crate::book::{self, Book, Song, SongRef};
use crate::default_project::DEFAULT_PROJECT;
use crate::music::Notation;
use crate::parser::Diagnostic;
use crate::parser::Parser;
use crate::parser::ParserConfig;
use crate::prelude::*;
use crate::render::tex_tools::TexConfig;
use crate::render::tex_tools::TexTools;
use crate::render::Renderer;
use crate::util::ExitStatusExt;

pub use toml::Value;

mod input;
use input::{InputSet, SongsGlobs};
mod output;
pub use output::{Format, Output};

pub type Metadata = BTreeMap<Box<str>, Value>;

type TomlMap = toml::map::Map<String, Value>;

fn dir_songs() -> PathBuf {
    "songs".into()
}

fn dir_templates() -> PathBuf {
    "templates".into()
}

fn dir_output() -> PathBuf {
    "output".into()
}

fn meta_default_chorus_label<'de, D>(de: D) -> Result<Metadata, D::Error>
where
    D: Deserializer<'de>,
{
    let mut meta = Metadata::deserialize(de)?;
    if !meta.contains_key("chorus_label") {
        meta.insert("chorus_label".into(), "Ch".into());
    }
    Ok(meta)
}

fn pathbuf_relative_only<'de, D>(de: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path = PathBuf::deserialize(de)?;
    if !path.is_relative() {
        let err = D::Error::custom(format!(
            "Configured paths must be relative to the project directory. Path: {:?}",
            path
        ));
        Err(err)
    } else {
        Ok(path)
    }
}

fn default_smart_punctuation() -> bool {
    true
}

#[derive(Deserialize, Debug)]
pub struct Settings {
    songs: SongsGlobs,

    #[serde(default = "dir_songs", deserialize_with = "pathbuf_relative_only")]
    dir_songs: PathBuf,
    #[serde(default = "dir_templates", deserialize_with = "pathbuf_relative_only")]
    dir_templates: PathBuf,
    #[serde(default = "dir_output", deserialize_with = "pathbuf_relative_only")]
    dir_output: PathBuf,

    #[serde(default)]
    pub notation: Notation,
    #[serde(default = "default_smart_punctuation")]
    pub smart_punctuation: bool,
    tex: Option<TexConfig>,

    pub output: Vec<Output>,
    #[serde(deserialize_with = "meta_default_chorus_label")]
    pub book: Metadata,
}

impl Settings {
    pub fn version() -> u32 {
        let major = env!("CARGO_PKG_VERSION_MAJOR");
        major.parse().unwrap()
    }

    pub fn from_file(path: &Path, project_dir: &Path) -> Result<Settings> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read project file {:?}", path))?;

        let parse_err = || format!("Could not parse project file {:?}", path);

        // Check version
        let settings: TomlMap = toml::from_str(&contents).with_context(parse_err)?;
        let version = settings.get("version").unwrap_or(&Value::Integer(1));
        let version = version
            .as_integer()
            .ok_or_else(|| anyhow!("'version' field expected to be an interger"))
            .with_context(parse_err)?;
        let self_ver = Self::version();
        if version < self_ver as _ {
            bail!(
                "This project was created with bard {}.x - to build with bard {}.x please follow the migration guide: https://bard.md/book/migration-{1}.html",
                version, self_ver);
        } else if version > self_ver as _ {
            bail!("This project was created with a newer version {}.x of bard, the project cannot be built by bard {}.x", version, self_ver);
        }

        let mut settings: Settings = toml::from_str(&contents).with_context(parse_err)?;

        settings.resolve(project_dir)?;
        Ok(settings)
    }

    pub fn dir_songs(&self) -> &Path {
        self.dir_songs.as_ref()
    }

    pub fn dir_output(&self) -> &Path {
        self.dir_output.as_ref()
    }

    fn resolve(&mut self, project_dir: &Path) -> Result<()> {
        self.dir_songs.resolve(project_dir);
        self.dir_templates.resolve(project_dir);
        self.dir_output.resolve(project_dir);

        for output in self.output.iter_mut() {
            output.resolve(&self.dir_templates, &self.dir_output)?;
        }

        Ok(())
    }
}

#[cfg(unix)]
static SCRIPT_EXT: &str = "sh";
#[cfg(windows)]
static SCRIPT_EXT: &str = "bat";

#[derive(Debug)]
pub struct Project {
    pub project_dir: PathBuf,
    pub settings: Settings,
    pub book: Book,

    project_file: PathBuf,
    input_paths: Vec<PathBuf>,
}

impl Project {
    pub fn new<P: AsRef<Path>>(app: &App, cwd: P) -> Result<Project> {
        let cwd = cwd.as_ref();
        let (project_file, project_dir) = Self::find_in_parents(cwd).ok_or_else(|| {
            anyhow!(
                "Could not find bard.toml file in current or parent directories\nCurrent directory: {:?}",
                cwd,
            )
        })?;

        app.status("Loading", format!("project at {:?}", project_dir));

        let settings = Settings::from_file(&project_file, &project_dir)?;
        let book = Book::new(&settings);

        let mut project = Project {
            project_file,
            project_dir,
            settings,
            input_paths: vec![],
            book,
        };

        project
            .load_md_files(app)
            .context("Failed to load input files")?;

        Ok(project)
    }

    fn find_in_parents(start_dir: &Path) -> Option<(PathBuf, PathBuf)> {
        assert!(start_dir.is_dir());

        let mut parent = start_dir;
        loop {
            let project_file = parent.join("bard.toml");
            if project_file.exists() {
                return Some((project_file, parent.into()));
            }

            parent = parent.parent()?;
        }
    }

    fn load_md_files(&mut self, app: &App) -> Result<()> {
        let input_set = InputSet::new(&self.settings.dir_songs)?;
        self.input_paths = self
            .settings
            .songs
            .iter()
            .try_fold(input_set, InputSet::apply_glob)?
            .finalize()?;

        let diag_sink = move |diag: Diagnostic| {
            app.parser_diag(diag);
        };

        for path in self.input_paths.iter() {
            app.check_interrupted()?;
            let source = fs::read_to_string(path)?;
            let config = ParserConfig::new(self.settings.notation, self.settings.smart_punctuation);
            let rel_path = path.strip_prefix(&self.project_dir).unwrap_or(path);
            let mut parser = Parser::new(&source, rel_path, config, diag_sink);
            let songs = parser
                .parse()
                .map_err(|_| anyhow!("Could not parse file {:?}", path))?;
            self.book.add_songs(songs);
        }

        self.book
            .postprocess(&self.settings.dir_output, app.img_cache())?;

        Ok(())
    }

    pub fn init<P: AsRef<Path>>(project_dir: P) -> Result<()> {
        DEFAULT_PROJECT.resolve(project_dir.as_ref()).create()
    }

    pub fn book_section(&self) -> &Metadata {
        &self.settings.book
    }

    pub fn songs(&self) -> &[Song] {
        &self.book.songs
    }

    pub fn songs_sorted(&self) -> &[SongRef] {
        &self.book.songs_sorted
    }

    fn run_script(&self, app: &App, output: &Output) -> Result<()> {
        let script_fn = match output.script.as_deref() {
            Some(s) => format!("{}.{}", s, SCRIPT_EXT),
            None => return Ok(()),
        };

        let script_path = self.settings.dir_output().join(&script_fn);
        if !script_path.exists() {
            bail!(
                "Could not find script file '{}' in the output directory.",
                script_fn
            );
        }

        app.status("Running", format!("script '{}'", script_fn));
        let mut child = Command::new(script_path)
            .current_dir(self.settings.dir_output())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .env("BARD", app.bard_exe())
            .env("OUTPUT", output.file.as_os_str())
            .env("OUTPUT_STEM", output.file.file_stem().unwrap()) // NB. unwrap is fine here, there's always a stem
            .env("PROJECT_DIR", self.project_dir.as_os_str())
            .env("OUTPUT_DIR", self.settings.dir_output().as_os_str())
            .spawn()?;
        app.child_wait(&mut child)?.into_result()?;

        Ok(())
    }

    pub fn render(&self, app: &App) -> Result<()> {
        fs::create_dir_all(&self.settings.dir_output)?;

        if self.settings.output.iter().any(|o| o.is_pdf()) {
            // Initialize Tex tools ahead of actual rendering so that
            // errors are reported early...
            TexTools::initialize(app, self.settings.tex.as_ref())
                .context("Could not initialize TeX tools.")?;
        }

        self.settings.output.iter().try_for_each(|output| {
            app.check_interrupted()?;
            app.status("Rendering", output.output_filename());
            let context = || {
                format!(
                    "Could not render output file {:?}",
                    output.file.file_name().unwrap()
                )
            };

            let renderer = Renderer::new(self, output, app.img_cache()).with_context(context)?;
            let tpl_version = renderer.version();

            let res = renderer.render(app).with_context(context).and_then(|_| {
                if app.post_process() {
                    self.run_script(app, output).with_context(|| {
                        format!(
                            "Could not run script for output file {:?}",
                            output.file.file_name().unwrap()
                        )
                    })
                } else {
                    Ok(())
                }
            });

            // Perform version check of the template (if the Render supports it and there is a template file).
            // This is done after rendering and preprocessing so that the CLI messages are at the bottom of the log.
            // Otherwise they tend to be far behind eg. TeX output etc.
            if let Some((tpl_version, tpl_path)) = tpl_version.zip(output.template.as_ref()) {
                book::version::compat_check(app, tpl_path, &tpl_version);
            }

            res
        })
    }

    pub fn input_paths(&self) -> &Vec<PathBuf> {
        &self.input_paths
    }

    pub fn output_paths(&self) -> impl Iterator<Item = &Path> {
        self.settings.output.iter().map(|o| o.file.as_path())
    }

    pub fn watch_paths(&self) -> impl Iterator<Item = &Path> {
        // Input MD files:
        // TODO: this won't work for wildcards
        let inputs = self.input_paths.iter().map(PathBuf::as_ref);

        // Templates:
        let templates = self
            .settings
            .output
            .iter()
            .filter_map(Output::template_path);

        // Images:
        let images = self.book.iter_images().map(|i| i.full_path());

        // bard.toml:
        iter::once(self.project_file.as_path())
            .chain(inputs)
            .chain(templates)
            .chain(images)
    }
}
