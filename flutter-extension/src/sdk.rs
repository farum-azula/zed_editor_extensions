//! The programs of the Dart and Flutter SDKs (and fvm), found the same way by every part of the
//! extension: on the PATH of the worktree's shell, under any of the names they have on the
//! platforms Zed runs on.

use zed_extension_api::Worktree;

/// A program on the PATH.
pub struct Executable {
    /// The command's name, as `fvm` takes it too (`fvm dart`, `fvm flutter`).
    pub name: &'static str,
    /// The file names it may have: `dart`, `dart.exe`, `dart.bat`.
    file_names: &'static [&'static str],
    /// What to do when it is missing.
    hint: &'static str,
}

pub const DART: Executable = Executable {
    name: "dart",
    file_names: &["dart", "dart.exe", "dart.bat"],
    hint: "install Flutter from flutter.dev (it comes with Dart), or Dart from dart.dev",
};

pub const FLUTTER: Executable = Executable {
    name: "flutter",
    file_names: &["flutter", "flutter.bat"],
    hint: "install it from flutter.dev",
};

pub const FVM: Executable = Executable {
    name: "fvm",
    file_names: &["fvm", "fvm.exe", "fvm.bat"],
    hint: "install it, or set \"useFvm\": false in the debug scenario",
};

impl Executable {
    /// The program's path on the worktree's PATH.
    pub fn find(&self, worktree: &Worktree) -> Result<String, String> {
        self.file_names
            .iter()
            .find_map(|file_name| worktree.which(file_name))
            .ok_or_else(|| self.missing())
    }

    fn missing(&self) -> String {
        format!("{} is not in PATH; {}", self.name, self.hint)
    }
}

/// Whether the project pins its Flutter with fvm (a `.fvmrc` in the worktree root): its programs
/// then run through fvm unless a scenario says otherwise.
pub fn project_pins_sdk_with_fvm(worktree: &Worktree) -> bool {
    worktree.read_text_file(".fvmrc").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_says_what_to_install() {
        assert_eq!(
            FLUTTER.missing(),
            "flutter is not in PATH; install it from flutter.dev"
        );
    }

    #[test]
    fn every_program_is_looked_up_under_its_own_name_first() {
        for executable in [DART, FLUTTER, FVM] {
            assert_eq!(executable.file_names[0], executable.name);
        }
    }
}
