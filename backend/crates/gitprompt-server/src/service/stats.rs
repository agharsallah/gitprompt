//! `GET /api/stats` (with group names for the dashboard), telemetry and the field catalog.

use indexmap::IndexMap;

use gitprompt_core::Error;
use gitprompt_core::spec::{SpecSet, SurfaceInfo};
use gitprompt_insights::catalog;
use gitprompt_insights::surface;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, Bucket, GroupBy};

use super::Service;
use crate::error::{ApiError, ApiResult};
use crate::model::{Catalog, Stats, StatsQuery, Telemetry};

/// Payloads read per field catalog (Python's `store.samples()` default).
const SAMPLES: usize = 5000;

impl Service {
    pub async fn stats(&self, query: StatsQuery) -> ApiResult<Stats> {
        if query.split_by.is_some() && query.group_by.is_none() {
            return Err(ApiError::Unprocessable("split_by needs group_by".into()));
        }
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let aggregate = AggregateQuery {
                filter: query.filter,
                group_by: query.group_by,
                split_by: query.split_by,
                order: query.order,
                limit: query.limit,
            };
            let mut result = inner.store.aggregate(&aggregate).map_err(|e| match e {
                Error::Invalid(message) => ApiError::Unprocessable(message),
                other => other.into(),
            })?;
            let names = GroupNames { set: &set, surfaces: surface::catalog(set.specs.values().map(AsRef::as_ref)) };
            names.name_groups(query.group_by, query.split_by, &mut result.groups);
            if query.group_by == Some(GroupBy::Session) {
                inner.name_sessions(&mut result.groups)?;
                inner.present_sessions(&set, &mut result.groups)?;
            }
            Ok(Stats { group_by: query.group_by, split_by: query.split_by, aggregate: result })
        })
        .await
    }

    pub async fn telemetry(&self, filter: PromptFilter) -> ApiResult<Telemetry> {
        self.run(move |inner| {
            inner.indexed()?;
            Ok(inner.store.telemetry(&filter)?)
        })
        .await
    }

    pub async fn fields(&self) -> ApiResult<Catalog> {
        self.run(|inner| {
            let set = inner.spec_set();
            Ok(catalog::build_catalog(&inner.store.samples(SAMPLES)?, set.specs.values().map(AsRef::as_ref)))
        })
        .await
    }
}

/// How the dashboard names a group: a `source` by its badge (spec `short_name`, unless the store
/// already gave a detail), a `surface` by its label.
struct GroupNames<'a> {
    set: &'a SpecSet,
    surfaces: IndexMap<String, SurfaceInfo>,
}

impl GroupNames<'_> {
    /// Name every group by `group_by` and each of its parts by `split_by`.
    fn name_groups(&self, group_by: Option<GroupBy>, split_by: Option<GroupBy>, groups: &mut [Bucket]) {
        for group in groups {
            self.name(group_by, group);
            group.parts.iter_mut().flatten().for_each(|part| self.name(split_by, part));
        }
    }

    /// Set `bucket.detail` to its name when `grouping` has one.
    fn name(&self, grouping: Option<GroupBy>, bucket: &mut Bucket) {
        let key = bucket.key.as_deref();
        let name = match grouping {
            Some(GroupBy::Source) if bucket.detail.is_none() => key.and_then(|k| self.set.get(k)).map(|spec| spec.label().to_owned()),
            Some(GroupBy::Surface) => self.surfaces.get(key.unwrap_or_default()).map(|info| info.name.clone()),
            _ => None,
        };
        if name.is_some() {
            bucket.detail = name;
        }
    }
}
