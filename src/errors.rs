use axoasset::AxoassetError;
use camino::Utf8PathBuf;
use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, OrandaError>;

#[derive(Debug, Diagnostic, Error)]
pub enum OrandaError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),

    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Syntect(#[from] syntect::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    AxoAsset(#[from] axoasset::AxoassetError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    AxoProject(#[from] axoproject::errors::AxoprojectError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Octolotl(#[from] octolotl::OctolotlError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Minijinja(#[from] minijinja::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),

    #[error(transparent)]
    Gazenot(#[from] gazenot::error::GazenotError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    GenerateCss(#[from] oranda_generate_css::errors::GenerateCssError),

    #[error("Failed to create a directory, `{dist_path}` to build your project in.")]
    DistDirCreationError {
        dist_path: String,
        #[source]
        details: std::io::Error,
    },

    #[error("Found an invalid value, `{path}`, assigned to ORANDA_CSS environment variable.")]
    #[diagnostic(help("Please make sure you give a valid path pointing to a css file."))]
    InvalidOrandaCSSOverride { path: String },

    #[error("Failed fetching releases from Github.")]
    GithubReleasesFetchError {
        #[source]
        details: octolotl::OctolotlError,
    },

    #[error("Failed fetching releases from axo Releases.")]
    AxoReleasesFetchError,

    #[error("Failed parsing response when fetching releases from Github.")]
    GithubReleaseParseError {
        #[source]
        details: axoasset::AxoassetError,
    },

    #[error("Your repository URL {url} couldn't be parsed.")]
    #[diagnostic(help("oranda only supports URLs you can also use with Git."))]
    UnknownRepoStyle { url: String },

    #[error("Could not find any releases from {repo_owner}/{repo_name} with a cargo-dist compatible `dist-manifest.json`.")]
    NoCargoDistReleasesFound {
        repo_owner: String,
        repo_name: String,
    },

    #[error(transparent)]
    FSExtra(#[from] fs_extra::error::Error),

    #[error("failed to read {filedesc} at {path}")]
    FileNotFound { filedesc: String, path: String },

    #[error("Could not find a build in {dist_dir}")]
    #[diagnostic(help("Did you remember to run `oranda build`?"))]
    BuildNotFound { dist_dir: String },

    #[error("Skipping malformed dist-manifest.json for {tag}")]
    #[diagnostic(severity = "warn")]
    CargoDistManifestMalformed {
        tag: String,
        #[diagnostic_source]
        details: AxoassetError,
    },

    #[error(
        "Failed checking for Github releases for repo, {repo}. Proceeding without releases..."
    )]
    #[diagnostic(severity = "warn")]
    ReleasesCheckFailed { repo: String },

    #[error("Skipping unparseable dist-manifest.json for {tag}")]
    #[diagnostic(help(
        "the schema was version {schema_version}, while our parser is version {parser_version}"
    ))]
    #[diagnostic(severity = "warn")]
    CargoDistManifestPartial {
        schema_version: String,
        parser_version: String,
        tag: String,
        #[diagnostic_source]
        details: AxoassetError,
    },

    #[error("Failed to parse package version {version}")]
    PackageVersionParse { version: String },

    #[error("Unable to create a path to {path} from root path {root_path}.")]
    #[diagnostic(help(
        "It can help to have your workspace members in a subdirectory under your workspace root."
    ))]
    PathdiffError { root_path: String, path: String },

    #[error("Couldn't load your mdbook at {path}")]
    MdBookLoad {
        path: String,
        #[source]
        details: mdbook::errors::Error,
    },

    #[error("Couldn't build your mdbook at {path}")]
    MdBookBuild {
        path: String,
        #[source]
        details: mdbook::errors::Error,
    },

    #[error("Can't build mdbook because book output directory {dest_path} is under book source directory {src_path}")]
    #[diagnostic(help(
        "Make sure that your book source does not contain your book output directory, as that will lead to infinite recursion. Change either the `src` setting or the `build_dir` setting in your book.toml."
    ))]
    MdbookBuildRecursive { src_path: String, dest_path: String },

    #[error("We found a potential {kind} project at {manifest_path} but there was an issue")]
    #[diagnostic(severity = "warn")]
    BrokenProject {
        kind: String,
        manifest_path: Utf8PathBuf,
        #[diagnostic_source]
        cause: axoproject::errors::AxoprojectError,
    },
    #[error("Unable to parse changelog for {name} version {version}")]
    #[diagnostic(help("Make sure that your changelog file lists the version in a header!"))]
    ChangelogParseFailed {
        name: String,
        version: String,
        #[source]
        details: axoproject::errors::AxoprojectError,
    },

    #[error("Failed to loading funding details at {path}")]
    #[diagnostic(severity = "warn")]
    FundingLoadFailed {
        path: Utf8PathBuf,
        #[diagnostic_source]
        details: axoasset::AxoassetError,
    },
    /// This error indicates we tried to deserialize some TOML with toml_edit
    /// but failed.
    #[error("Failed to edit toml document")]
    TomlEdit {
        /// The SourceFile we were trying to parse
        #[source_code]
        source: axoasset::SourceFile,
        /// The range the error was found on
        #[label]
        span: Option<miette::SourceSpan>,
        /// Details of the error
        #[source]
        details: toml_edit::TomlError,
    },

    #[error("We were unable to watch your filesystem for changes")]
    #[diagnostic(help = "Make sure that oranda has privileges to set up file watchers!")]
    FilesystemWatchError(#[from] notify_debouncer_mini::notify::Error),

    #[error("Failed to fetch your funding info from GitHub.")]
    #[diagnostic(help = "Make sure that your funding file is located at `.github/FUNDING.yml`.")]
    GithubFundingFetchError {
        #[source]
        details: reqwest::Error,
    },

    #[error("Couldn't find your FUNDING.yml or funding.md")]
    #[diagnostic(
        help = "You can manually specify md_path or yml_path in your components.funding config"
    )]
    FundingConfigInvalid,

    #[error("Error while parsing FUNDING.yml")]
    #[diagnostic(
        help = "Make sure your FUNDING.yml conforms to GitHub's format!",
        url = "https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/displaying-a-sponsor-button-in-your-repository"
    )]
    GithubFundingParseError { details: String },

    #[error("Your preferred_funding '{preferred}' didn't match any of the sources we found")]
    #[diagnostic(help = "{help}")]
    PreferredFundingNotFound { preferred: String, help: String },

    #[error("Couldn't find your book.toml")]
    #[diagnostic(help = "You can manually specify path in your components.mdbook config")]
    MdBookConfigInvalid,

    #[error("Specified path `{path}` was not found on your filesystem!")]
    #[diagnostic(
        help = "Make sure you specify your path relative to the oranda.json/manifest file/README file of your project!"
    )]
    PathDoesNotExist { path: String },

    #[error("{0}")]
    Other(String),
}
