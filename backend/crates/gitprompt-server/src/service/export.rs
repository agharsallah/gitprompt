//! Exports: a session, a project, the overview or a prompt list, rendered in one format, and the
//! charts an export has.

use gitprompt_export::{Context, ExportError, Request, Zone, charts, html, naming};
use gitprompt_store::filter::PromptFilter;

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::{ChartInfo, ExportFile, ExportKind, ExportQuery};

impl Inner {
    fn export(&self, kind: ExportKind, query: &ExportQuery, mut filter: PromptFilter) -> ApiResult<gitprompt_export::Export> {
        let (set, _) = self.indexed()?;
        filter.before = None; // an export covers every page
        let id = || query.id.clone().ok_or_else(|| ApiError::Unprocessable(format!("a {} export needs an id", kind_name(kind))));
        let request = match kind {
            ExportKind::Session => Request::Session { id: id()? },
            ExportKind::Project => Request::Project { id: id()? },
            ExportKind::Overview => Request::Overview { filter },
            ExportKind::Prompts => Request::Prompts { filter },
        };
        let context = Context {
            home: Some(self.settings.user_home.display().to_string()),
            roots: std::collections::BTreeSet::default(),
            zone: Zone::Local,
        };
        let generated_at = self.local_now().strftime("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string();
        gitprompt_export::build_from_store_with(&self.store, &set.specs, &request, &query.options(), &context, &generated_at)
            .map_err(export_error)
    }

    fn local_now(&self) -> jiff::Zoned {
        self.now().to_zoned(jiff::tz::TimeZone::system())
    }
}

impl Service {
    /// `filter` applies to `overview` and `prompts`. An unknown chart id renders the first chart.
    pub async fn export(&self, kind: ExportKind, query: ExportQuery, filter: PromptFilter) -> ApiResult<ExportFile> {
        self.run(move |inner| {
            let export = inner.export(kind, &query, filter)?;
            let format = gitprompt_export::Format::from(query.format);
            let body = gitprompt_export::render(&export, format, query.chart.as_deref()).map_err(export_error)?;
            Ok(ExportFile {
                media_type: format.media_type().to_owned(),
                filename: naming::filename(&export.report, format, inner.local_now().date()),
                attachment: query.download,
                csp: (format == gitprompt_export::Format::Html).then(|| html::CSP.to_owned()),
                body,
            })
        })
        .await
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
