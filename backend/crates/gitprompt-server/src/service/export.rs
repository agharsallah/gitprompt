//! Exports: a session, a project, the overview or a prompt list, rendered in one format, the
//! charts an export has, and the prompts behind a branch.

use gitprompt_export::{Context, Export, ExportError, Format, Request, Zone, charts, html, naming};
use gitprompt_store::filter::PromptFilter;

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::{BranchQuery, ChartInfo, ExportFile, ExportKind, ExportQuery};

impl Inner {
    fn export(&self, kind: ExportKind, query: &ExportQuery, mut filter: PromptFilter) -> ApiResult<Export> {
        let (set, _) = self.indexed()?;
        filter.before = None; // an export covers every page
        let id = || query.id.clone().ok_or_else(|| ApiError::Unprocessable(format!("a {} export needs an id", kind_name(kind))));
        let request = match kind {
            ExportKind::Session => Request::Session { id: id()? },
            ExportKind::Project => Request::Project { id: id()? },
            ExportKind::Overview => Request::Overview { filter },
            ExportKind::Prompts => Request::Prompts { filter },
        };
        let context = self.export_context();
        gitprompt_export::build_from_store_with(&self.store, &set.specs, &request, &query.options(), &context, &self.generated_at())
            .map_err(export_error)
    }

    /// The project's story (read with this app's git), and from it the prompts on the branch.
    fn branch_export(&self, query: &BranchQuery) -> ApiResult<Export> {
        let story = self.project_story(&query.project)?;
        gitprompt_export::branch_export(&story, &query.branch, &self.export_context(), &self.generated_at()).map_err(export_error)
    }

    /// Where exports are made: this user's home (shown as `~`), in this computer's zone.
    fn export_context(&self) -> Context {
        Context {
            home: Some(self.settings.user_home.display().to_string()),
            roots: std::collections::BTreeSet::default(),
            zone: Zone::Local,
        }
    }

    /// The app clock's now, local, as an export states it.
    fn generated_at(&self) -> String {
        self.local_now().strftime("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string()
    }

    fn local_now(&self) -> jiff::Zoned {
        self.now().to_zoned(jiff::tz::TimeZone::system())
    }

    /// The export rendered in `format`, with what its headers say.
    fn file(&self, export: &Export, format: Format, chart: Option<&str>, attachment: bool) -> ApiResult<ExportFile> {
        let body = gitprompt_export::render(export, format, chart).map_err(export_error)?;
        Ok(ExportFile {
            media_type: format.media_type().to_owned(),
            filename: naming::filename(&export.report, format, self.local_now().date()),
            attachment,
            csp: (format == Format::Html).then(|| html::CSP.to_owned()),
            body,
        })
    }
}

impl Service {
    /// `filter` applies to `overview` and `prompts`. An unknown chart id renders the first chart.
    pub async fn export(&self, kind: ExportKind, query: ExportQuery, filter: PromptFilter) -> ApiResult<ExportFile> {
        self.run(move |inner| {
            let export = inner.export(kind, &query, filter)?;
            inner.file(&export, Format::from(query.format), query.chart.as_deref(), query.download)
        })
        .await
    }

    /// The prompts you typed on a branch of a project, as Markdown to paste into a pull request
    /// (`gitprompt_export::branch_export`: secrets masked, paths relative). A branch without any
    /// is still a block, which says so; an unknown project is not found.
    pub async fn export_branch(&self, query: BranchQuery) -> ApiResult<ExportFile> {
        self.run(move |inner| inner.file(&inner.branch_export(&query)?, Format::Markdown, None, false)).await
    }

    /// The charts of an export that have data.
    pub async fn export_charts(&self, kind: ExportKind, query: ExportQuery, filter: PromptFilter) -> ApiResult<Vec<ChartInfo>> {
        self.run(move |inner| {
            let export = inner.export(kind, &query, filter)?;
            Ok(charts::chart_list(&export.report).into_iter().map(|c| ChartInfo { id: c.id, title: c.title }).collect())
        })
        .await
    }
}

fn kind_name(kind: ExportKind) -> &'static str {
    gitprompt_export::Kind::from(kind).as_str()
}

/// No chart with data is a 404 like an unknown id; everything else keeps its domain status.
fn export_error(error: ExportError) -> ApiError {
    match error {
        ExportError::NoChart => ApiError::NotFound(error.to_string()),
        other => ApiError::Domain(other.into()),
    }
}
