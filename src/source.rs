use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{
    builder::CrateType as ChosenCrateType,
    error::{BuildErrorKind, Result, ResultExt},
};

#[derive(Hash, Clone, Debug)]
pub enum CrateType {
    Library,
    Binary,
    Mixed,
}

#[derive(Hash, Clone, Debug)]
/// Information about CUDA crate.
pub struct Crate {
    name: String,
    path: PathBuf,
    output_file_prefix: String,
    crate_type: CrateType,
}

impl Crate {
    /// Try to locate a crate at the `path` and collect needed information.
    pub fn analyse<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = {
            env::current_dir()
                .context(BuildErrorKind::OtherError)?
                .join(&path)
        };

        match fs::metadata(path.join("Cargo.toml")) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    bail!(BuildErrorKind::InvalidCratePath(path.clone()));
                }
            }

            Err(_) => {
                bail!(BuildErrorKind::InvalidCratePath(path.clone()));
            }
        }

        let cargo_toml: toml::Value = {
            let mut reader = BufReader::new(
                fs::File::open(path.join("Cargo.toml")).context(BuildErrorKind::OtherError)?,
            );

            let mut contents = String::new();

            reader
                .read_to_string(&mut contents)
                .context(BuildErrorKind::OtherError)?;

            toml::from_str(&contents).context(BuildErrorKind::OtherError)?
        };

        let cargo_toml_name = match cargo_toml["package"]["name"].as_str() {
            Some(name) => name,
            None => {
                bail!(BuildErrorKind::InternalError(String::from(
                    "Cannot get crate name"
                )));
            }
        };

        let is_library =
            cargo_toml.get("lib").is_some() || path.join("src").join("lib.rs").exists();
        let is_binary =
            cargo_toml.get("bin").is_some() || path.join("src").join("main.rs").exists();

        let output_file_prefix = cargo_toml_name.replace('-', "_");

        let crate_type = match (is_binary, is_library) {
            (false, true) => CrateType::Library,
            (true, false) => CrateType::Binary,
            (true, true) => CrateType::Mixed,
            (false, false) => {
                bail!(BuildErrorKind::InternalError(
                    "Unable to find neither `src/lib.rs` nor `src/main.rs` \
                    nor a [lib] nor [[bin]] section in `Cargo.toml`"
                        .into()
                ));
            }
        };

        Ok(Crate {
            name: cargo_toml_name.to_string(),
            path,
            output_file_prefix,
            crate_type,
        })
    }

    /// Returns PTX assmbly filename prefix.
    pub fn get_output_file_prefix(&self) -> &str {
        &self.output_file_prefix
    }

    /// Returns the crate type to build the PTX with
    pub fn get_crate_type(&self, crate_type: Option<ChosenCrateType>) -> Result<ChosenCrateType> {
        match (&self.crate_type, crate_type) {
            (CrateType::Library, Some(ChosenCrateType::Library) | None)
            | (CrateType::Mixed, Some(ChosenCrateType::Library)) => Ok(ChosenCrateType::Library),

            (CrateType::Binary, Some(ChosenCrateType::Binary) | None)
            | (CrateType::Mixed, Some(ChosenCrateType::Binary)) => Ok(ChosenCrateType::Binary),

            (CrateType::Mixed, None) => {
                bail!(BuildErrorKind::MissingCrateType);
            }

            (CrateType::Library, Some(ChosenCrateType::Binary)) => {
                bail!(BuildErrorKind::InvalidCrateType("Binary".into()));
            }

            (CrateType::Binary, Some(ChosenCrateType::Library)) => {
                bail!(BuildErrorKind::InvalidCrateType("Library".into()));
            }
        }
    }

    /// Returns crate name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns crate root path.
    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }

    /// Returns temporary crate build location that can be `cargo clean`ed.
    pub fn get_output_path(&self) -> Result<PathBuf> {
        let mut path = PathBuf::from(env!("OUT_DIR"));

        path.push(&self.output_file_prefix);
        path.push(format!("{:x}", self.get_hash()));

        fs::create_dir_all(&path).context(BuildErrorKind::OtherError)?;
        Ok(path)
    }

    fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);

        hasher.finish()
    }
}

#[test]
fn should_find_crate_names() {
    let source = Crate::analyse("tests/fixtures/sample-crate").unwrap();

    assert_eq!(source.get_output_file_prefix(), "sample_ptx_crate");
}

#[test]
fn should_find_app_crate_names() {
    let source = Crate::analyse("tests/fixtures/app-crate").unwrap();

    assert_eq!(source.get_output_file_prefix(), "sample_app_ptx_crate");
}

#[test]
fn should_find_mixed_crate_names() {
    let source = Crate::analyse("tests/fixtures/mixed-crate").unwrap();

    assert_eq!(source.get_output_file_prefix(), "mixed_crate");
}

#[test]
fn should_check_existence_of_crate_path() {
    let result = Crate::analyse("tests/fixtures/non-existing-crate");

    match result.unwrap_err().kind() {
        BuildErrorKind::InvalidCratePath(path) => {
            assert!(path.ends_with("tests/fixtures/non-existing-crate"));
        }

        _ => unreachable!("it should fail with proper error"),
    }
}

#[test]
fn should_check_validity_of_crate_path() {
    let result = Crate::analyse("tests/builder.rs");

    match result.unwrap_err().kind() {
        BuildErrorKind::InvalidCratePath(path) => {
            assert!(path.ends_with("tests/builder.rs"));
        }

        _ => unreachable!("it should fail with proper error"),
    }
}

#[test]
fn should_provide_output_path() {
    let source_crate = Crate::analyse("tests/fixtures/sample-crate").unwrap();

    assert!(source_crate
        .get_output_path()
        .unwrap()
        .starts_with(Path::new(env!("OUT_DIR")).join("sample_ptx_crate")));
}
