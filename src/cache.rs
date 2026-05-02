use camino::Utf8PathBuf;

use crate::model::NormalizedProject;

pub fn cache_dir(project: &NormalizedProject) -> Utf8PathBuf {
    project.project_dir.join(".cutline").join("cache")
}
