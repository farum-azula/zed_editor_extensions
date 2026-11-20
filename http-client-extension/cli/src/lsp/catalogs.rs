//! The catalog lenses and code actions use: the built-in one, or built-in +
//! `<root>/.zed/http/frameworks.json`, reloaded when that file changes.

use crate::catalog::{self, Catalog};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// The project's catalog and the modification time of the file it was loaded from.
struct ProjectCatalog {
    modified: SystemTime,
    catalog: Catalog,
}

#[derive(Default)]
pub(super) struct Catalogs {
    project: Option<ProjectCatalog>,
}

impl Catalogs {
    pub fn get(&mut self, root: Option<&Path>) -> &Catalog {
        let Some(root) = root else { return Catalog::builtin() };
        let file = catalog::project_file(root);
        let Some(modified) = fs::metadata(&file).and_then(|metadata| metadata.modified()).ok() else {
            return Catalog::builtin();
        };
        let cached = self.project.take().filter(|project| project.modified == modified);
        let project = match cached {
            Some(project) => project,
            None => match Catalog::for_root(root) {
                Ok(catalog) => ProjectCatalog { modified, catalog },
                Err(error) => {
                    // Shown in Zed's language server log; keep working with the built-in catalog.
                    eprintln!("zed-http: {error}");
                    return Catalog::builtin();
                }
            },
        };
        &self.project.insert(project).catalog
    }
}
