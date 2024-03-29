use bard::book;
use bard::prelude::*;
use bard::project::Project;
use bard::render::Renderer;

use bard::util::ImgCache;
use semver::Version;

mod util;
pub use util::*;

#[track_caller]
fn get_output_versions(project: &Project) -> Vec<(Version, PathBuf)> {
    // Imperative code so that track_caller works
    let mut res = vec![];
    for o in &project.settings.output {
        let renderer = Renderer::new(project, o, &ImgCache::new()).unwrap();
        if let Some(ver) = renderer.version() {
            res.push((ver, o.file.clone()));
        }
    }
    res
}

#[track_caller]
fn assert_project_versions(project: &Project) {
    for (ver, output) in get_output_versions(project) {
        assert_eq!(&ver, book::version::current(), "{:?}", output);
    }
}

#[test]
fn ast_version_check_load() {
    let build = Builder::build(TEST_PROJECTS / "ast-version-check").unwrap();

    let expected = Version::new(1, 2, 3);
    for (ver, output) in get_output_versions(&build.project) {
        assert_eq!(ver, expected, "{:?}", output);
    }
}

#[test]
fn ast_version_check_default_project() {
    let build = Builder::build_with_name(ROOT / "default", "ast-version-check-default").unwrap();
    assert_project_versions(&build.project);
}

#[test]
fn ast_version_check_example_project() {
    let build = Builder::build_with_name(ROOT / "example", "ast-version-check-example").unwrap();
    assert_project_versions(&build.project);
}
