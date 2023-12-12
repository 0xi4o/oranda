use std::path::Path;

use axoasset::{Asset, LocalAsset};
use axoproject::GithubRepo;
use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use minijinja::context;
use tracing::instrument;

use crate::config::{AxoprojectLayer, Config, ReleasesSource};
use crate::data::github::GithubRelease;
use crate::data::{funding::Funding, workspaces, Context};
use crate::errors::*;

use crate::data::workspaces::WorkspaceData;
use crate::site::templates::Templates;
use crate::site::workspace_index::WorkspaceIndexContext;
use layout::css;
pub use layout::javascript;
use page::Page;

pub mod artifacts;
pub mod changelog;
pub mod funding;
pub mod layout;
pub mod link;
pub mod markdown;
pub mod mdbook;
pub mod oranda_theme;
pub mod page;
pub mod rss;
pub mod templates;
mod workspace_index;

#[derive(Debug)]
pub struct Site {
    pub workspace_data: Option<WorkspaceData>,
    pub pages: Vec<Page>,
}

impl Site {
    pub fn build_multi(workspace_config: &Config, json_only: bool) -> Result<Vec<Site>> {
        tracing::info!("Workspace detected, gathering info...");
        // We assume the root path is wherever oranda-workspace.json is located (current dir)
        let root_path = Utf8PathBuf::from_path_buf(std::env::current_dir()?).unwrap_or_default();

        let mut workspace_config_path = root_path.clone();
        workspace_config_path.push("oranda-workspace.json");
        let mut results = Vec::new();
        let members =
            workspaces::from_config(workspace_config, &root_path, &workspace_config_path)?;
        tracing::info!("Building {} workspace member(s)...", members.len());
        for member in &members {
            std::env::set_current_dir(&member.path)?;
            let mut site = if json_only {
                Self::build_single_json_only(&member.config, Some(member.slug.to_string()))?
            } else {
                Self::build_single(&member.config, Some(member.slug.to_string()))?
            };
            site.workspace_data = Some(member.clone());
            results.push(site);
            std::env::set_current_dir(&root_path)?;
        }

        Ok(results)
    }

    pub fn build_and_write_workspace_index(
        workspace_config: &Config,
        member_data: &Vec<WorkspaceData>,
    ) -> Result<()> {
        let templates = Templates::new_for_workspace_index(workspace_config)?;
        if workspace_config.styles.favicon.is_none() {
            layout::header::place_default_favicon(workspace_config)?;
        }
        css::place_css(
            &workspace_config.build.dist_dir,
            &workspace_config.styles.oranda_css_version,
        )?;
        let context = WorkspaceIndexContext::new(member_data, workspace_config)?;
        let page = Page::new_from_template(
            "index.html",
            &templates,
            "workspace_index/index.html",
            &context,
        )?;
        let mut dist = Utf8PathBuf::from(&workspace_config.build.dist_dir);
        let additional_css = &workspace_config.styles.additional_css;
        if !additional_css.is_empty() {
            css::write_additional_css(additional_css, &dist)?;
        }
        dist.push("index.html");
        LocalAsset::write_new_all(&page.contents, dist)?;
        Ok(())
    }

    #[instrument("workspace_page", fields(prefix = prefix))]
    pub fn build_single(config: &Config, prefix: Option<String>) -> Result<Site> {
        Self::clean_dist_dir(&config.build.dist_dir)?;
        if config.styles.favicon.is_none() {
            layout::header::place_default_favicon(config)?;
        }
        css::place_css(&config.build.dist_dir, &config.styles.oranda_css_version)?;
        let needs_context = Self::needs_context(config)?;
        let context = if needs_context {
            Some(Self::build_context(config)?)
        } else {
            None
        };

        let templates = Templates::new(config, context.as_ref())?;

        let mut pages = vec![];

        if !config.build.additional_pages.is_empty() {
            let mut additional_pages =
                Self::build_additional_pages(&config.build.additional_pages, &templates, config)?;
            pages.append(&mut additional_pages);
        }

        let mut index = None;
        Self::print_plan(config);

        if let Some(mut context) = context {
            if config.components.artifacts_enabled() {
                if let Some(latest) = context.latest_mut() {
                    // Give especially nice treatment to the latest release and make
                    // its scripts easy to view (others get hotlinked and will just download)
                    latest.artifacts.make_scripts_viewable(config)?;

                    let template_context = artifacts::template_context(&context, config)?;
                    index = Some(Page::new_from_both(
                        &config.project.readme_path,
                        "index.html",
                        &templates,
                        "index.html",
                        context!(artifacts => template_context),
                        config,
                    )?);
                    let artifacts_page = Page::new_from_template(
                        "artifacts.html",
                        &templates,
                        "artifacts.html",
                        &template_context,
                    )?;
                    pages.push(artifacts_page);
                    if let Some(template_context) = template_context {
                        artifacts::write_artifacts_json(config, &template_context)?;
                    }
                }
            }
            if config.components.changelog.is_some() {
                let mut changelog_pages =
                    Self::build_changelog_pages(&context, &templates, config)?;
                pages.append(&mut changelog_pages);
            }
            if let Some(funding_cfg) = &config.components.funding {
                let funding = Funding::new(funding_cfg, &config.styles)?;
                let context = funding::context(funding_cfg, &funding)?;
                let page =
                    Page::new_from_template("funding.html", &templates, "funding.html", &context)?;
                pages.push(page);
            }
        }

        let index = if let Some(index) = index {
            index
        } else {
            Page::new_from_both(
                &config.project.readme_path,
                "index.html",
                &templates,
                "index.html",
                context!(),
                config,
            )?
        };
        pages.push(index);
        Ok(Site {
            pages,
            workspace_data: None,
        })
    }

    #[instrument("workspace_page", fields(prefix = prefix))]
    pub fn build_single_json_only(config: &Config, prefix: Option<String>) -> Result<Site> {
        Self::clean_dist_dir(&config.build.dist_dir)?;
        let context = if Self::needs_context(config)? {
            Some(Self::build_context(config)?)
        } else {
            None
        };

        if let Some(mut context) = context {
            if config.components.artifacts_enabled() {
                if let Some(latest) = context.latest_mut() {
                    latest.artifacts.make_scripts_viewable(config)?;
                    let template_context = artifacts::template_context(&context, config)?;
                    if let Some(template_context) = template_context {
                        artifacts::write_artifacts_json(config, &template_context)?;
                    }
                }
            }
        }

        Ok(Site {
            pages: vec![],
            workspace_data: None,
        })
    }

    pub fn get_workspace_config() -> Result<Option<Config>> {
        let path = Utf8PathBuf::from("./oranda-workspace.json");
        if path.exists() {
            let workspace_config = Config::build_workspace_root(&path)?;
            Ok(Some(workspace_config))
        } else {
            Ok(None)
        }
    }

    fn needs_context(config: &Config) -> Result<bool> {
        Ok(config.project.repository.is_some()
            && (config.components.artifacts_enabled()
                || config.components.changelog.is_some()
                || config.components.funding.is_some()
                || Self::has_repo_and_releases(&config.project.repository)?))
    }

    fn has_repo_and_releases(repo_config: &Option<String>) -> Result<bool> {
        if let Some(repo) = repo_config {
            GithubRelease::repo_has_releases(&GithubRepo::from_url(repo)?)
        } else {
            Ok(false)
        }
    }

    fn print_plan(config: &Config) {
        let mut planned_components = Vec::new();
        if config.components.artifacts_enabled() {
            planned_components.push("artifacts");
        }
        if config.components.changelog.is_some() {
            planned_components.push("changelog");
        }
        if config.components.funding.is_some() {
            planned_components.push("funding");
        }
        if config.components.mdbook.is_some() {
            planned_components.push("mdbook");
        }

        let joined = planned_components
            .iter()
            .fold(String::new(), |acc, component| {
                if acc.is_empty() {
                    component.to_string()
                } else {
                    format!("{}, {}", acc, component)
                }
            });
        if !joined.is_empty() {
            tracing::info!("Building components: {}", joined);
        }
    }

    fn build_context(config: &Config) -> Result<Context> {
        let Some(repo_url) = config.project.repository.as_ref() else {
            return Context::new_current(&config.project, config.components.artifacts.as_ref());
        };
        let maybe_ctx = match config.components.source {
            Some(ReleasesSource::GitHub) | None => Context::new_github(
                repo_url,
                &config.project,
                config.components.artifacts.as_ref(),
            ),
            Some(ReleasesSource::Axodotdev) => Context::new_axodotdev(
                &config.project.name,
                repo_url,
                &config.project,
                config.components.artifacts.as_ref(),
            ),
        };

        match maybe_ctx {
            Ok(c) => Ok(c),
            Err(e) => {
                // We don't want to hard error here, as we can most likely keep on going even
                // without a well-formed context.
                eprintln!("{:?}", miette::Report::new(e));
                Ok(Context::new_current(
                    &config.project,
                    config.components.artifacts.as_ref(),
                )?)
            }
        }
    }

    fn build_additional_pages(
        files: &IndexMap<String, String>,
        templates: &Templates,
        config: &Config,
    ) -> Result<Vec<Page>> {
        let mut pages = vec![];
        for file_path in files.values() {
            if page::source::is_markdown(file_path) {
                let additional_page = Page::new_from_markdown(file_path, templates, config, true)?;
                pages.push(additional_page)
            } else {
                let msg = format!(
                    "File {} in additional pages is not markdown and will be skipped",
                    file_path
                );
                tracing::warn!("{}", &msg);
            }
        }
        Ok(pages)
    }

    fn build_changelog_pages(
        context: &Context,
        templates: &Templates,
        config: &Config,
    ) -> Result<Vec<Page>> {
        let mut pages = vec![];
        // Recompute the axoproject layer here (unfortunately we don't pass it around)
        let cur_dir = std::env::current_dir()?;
        let project = AxoprojectLayer::get_best_workspace(
            &Utf8PathBuf::from_path_buf(cur_dir).expect("Current directory isn't UTF-8?"),
        );
        let index_context = changelog::index_context(context, config, project.as_ref())?;
        let changelog_page = Page::new_from_template(
            "changelog.html",
            templates,
            "changelog_index.html",
            &index_context,
        )?;
        pages.push(changelog_page);
        if config
            .components
            .changelog
            .clone()
            .is_some_and(|c| c.rss_feed)
        {
            let changelog_rss = rss::generate_rss_feed(&index_context, config)?;
            pages.push(Page {
                contents: changelog_rss.to_string(),
                filename: "changelog.rss".to_string(),
            });
        }
        if !(context.releases.len() == 1 && context.releases[0].source.is_current_state()) {
            for release in context.releases.iter() {
                let single_context = changelog::single_context(release, config, project.as_ref());
                let page = Page::new_from_template(
                    &format!("changelog/{}.html", single_context.version_tag),
                    templates,
                    "changelog_single.html",
                    &context!(release => single_context),
                )?;
                pages.push(page);
            }
        }
        Ok(pages)
    }

    pub fn copy_static(dist_dir: &Utf8Path, static_path: &str) -> Result<()> {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        // We want to be able to rename dirs in the copy, this enables it
        options.copy_inside = true;
        fs_extra::copy_items(&[static_path], dist_dir, &options)?;

        Ok(())
    }

    /// Properly writes page data to disk.
    /// This takes an optional config argument, the presence of which indicates that we're building
    /// a single site. If the config isn't given, it indicates that we're building a workspace member
    /// page instead (its config is stored in the `Site` struct itself). If none of these apply,
    /// that's a bug (for now).
    pub fn write(self, config: Option<&Config>) -> Result<()> {
        // Differentiate between workspace page write or single page write by checking if there's a
        // workspace config set in the struct, or if the (single) page config is manually passed to
        // the function.
        let config = if let Some(config) = config {
            config
        } else {
            &self.workspace_data.as_ref().expect("Attempted to build workspace page without workspace config. This is an oranda bug!").config
        };
        let dist = Utf8PathBuf::from(&config.build.dist_dir);
        for page in self.pages {
            let filename_path = Utf8PathBuf::from(&page.filename);
            // Prepare to write a "pretty link" for pages that aren't index.html already.
            // This essentially means that we rewrite the page from "page.html" to
            // "page/index.html", so that it can be loaded as "mysite.com/page" in the browser.
            let full_path: Utf8PathBuf = if !filename_path.ends_with("index.html")
                && filename_path.extension() == Some("html")
            {
                // Surely we can't we do anything BUT unwrap here? A file without a name is a mess.
                let file_stem = filename_path.file_stem().expect("missing file_stem???");
                let parent = filename_path.parent().unwrap_or("".into());
                dist.join(parent).join(file_stem).join("index.html")
            } else {
                dist.join(filename_path)
            };
            LocalAsset::write_new_all(&page.contents, full_path)?;
        }
        if let Some(book_cfg) = &config.components.mdbook {
            mdbook::build_mdbook(
                self.workspace_data.as_ref(),
                &dist,
                book_cfg,
                &config.styles.theme,
                &config.styles.syntax_theme,
            )?;
        }
        if let Some(origin_path) = config.styles.favicon.as_ref() {
            let copy_result_future = Asset::copy(origin_path, &config.build.dist_dir[..]);
            tokio::runtime::Handle::current().block_on(copy_result_future)?;
        }
        if Path::new(&config.build.static_dir).exists() {
            Self::copy_static(&dist, &config.build.static_dir)?;
        }
        javascript::write_os_script(&dist)?;

        let additional_css = &config.styles.additional_css;
        if !additional_css.is_empty() {
            css::write_additional_css(additional_css, &dist)?;
        }

        Ok(())
    }

    pub fn clean_dist_dir(dist_path: &str) -> Result<()> {
        if Path::new(dist_path).exists() {
            std::fs::remove_dir_all(dist_path)?;
        }
        match std::fs::create_dir_all(dist_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(OrandaError::DistDirCreationError {
                dist_path: dist_path.to_string(),
                details: e,
            }),
        }
    }
}
