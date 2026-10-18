use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use utoipa::ToSchema;
use validify::{Validate, ValidationError, field_err};

/// Used to obtain paginated lists with a total number of items in
/// the tables.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct List<T> {
    pub total: usize,
    pub items: Vec<T>,
}

impl<T> List<T> {
    pub fn new(total: usize, items: Vec<T>) -> Self {
        Self { total, items }
    }
}

impl<T> std::iter::IntoIterator for List<T> {
    type Item = T;

    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Used to paginate queries.
///
/// `page` defaults to 1 (which results in offset 0).
/// `per_page` defaults to 10.
#[serde_as]
#[derive(Debug, Clone, Copy, Deserialize, Validate, utoipa::ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// The limit.
    #[serde_as(as = "DisplayFromStr")]
    #[validate(range(min = 1.))]
    per_page: usize,

    /// The offset.
    #[serde_as(as = "DisplayFromStr")]
    #[validate(range(min = 1.))]
    page: usize,
}

impl Pagination {
    /// `page` is 1-based.
    #[cfg(test)]
    pub fn new(page: usize, per_page: usize) -> Self {
        Self { page, per_page }
    }

    /// Returns a tuple whose first element is the LIMIT and second
    /// the OFFSET for the query.
    pub fn to_limit_offset(self) -> (i64, i64) {
        let Self { page, per_page } = self;
        (per_page as i64, ((page - 1) * per_page) as i64)
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            per_page: 10,
            page: 1,
        }
    }
}

/// Used to paginate queries and sort the rows.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PaginationSort<Srt: ToSchema, Srch: ToSchema = ()> {
    /// See [Pagination].
    #[validate]
    #[serde(flatten)]
    pub pagination: Option<Pagination>,

    /// The column to sort by.
    ///
    /// Default: `updated_at`
    #[serde(flatten)]
    pub sort: Option<Sort<Srt>>,

    #[validate]
    #[serde(flatten)]
    pub search: Option<Search<Srch>>,
}

impl<Srt: ToSchema, Srch: ToSchema> PaginationSort<Srt, Srch> {
    /// See [Pagination::to_limit_offset].
    pub fn limit_offset(&self) -> (i64, i64) {
        self.pagination
            .map(|pagination| pagination.to_limit_offset())
            .unwrap_or(Pagination::default().to_limit_offset())
    }
}

impl<Srt: ToSchema + ToSortColumn, Srch: ToSchema> PaginationSort<Srt, Srch> {
    /// Returns a tuple whose first element is the sort column and
    /// second the sort direction ASC/DESC.
    pub fn sort(&self) -> (&str, &str) {
        self.sort
            .as_ref()
            .map(|sort| sort.sort())
            .unwrap_or(Sort::default().sort())
    }
}

impl<Srt: ToSchema, Srch: ToSchema> Default for PaginationSort<Srt, Srch> {
    fn default() -> Self {
        Self {
            pagination: Some(Pagination::default()),
            sort: None,
            search: None,
        }
    }
}

/// Struct used for search functionality when querying various models.
#[derive(Debug, Clone, Validate, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Sort<T> {
    #[serde(rename = "sortColumn", alias = "sort.by", alias = "sort.column")]
    pub column: T,

    /// The column to search by when performing the query. This is intended to be an enum of all
    /// the possible search values for a given query.
    #[serde(rename = "sortDirection", alias = "sort.direction", alias = "sort.dir")]
    pub direction: SortDirection,
}

impl<T: ToSortColumn> Sort<T> {
    pub fn sort(&self) -> (&'static str, &'static str) {
        (self.column.to_sort_column(), self.direction.as_str())
    }
}

crate::sort_column! {
    DefaultSort,
    UpdatedAt => "updated_at"
}

impl Default for Sort<DefaultSort> {
    fn default() -> Self {
        Self {
            column: DefaultSort::UpdatedAt,
            direction: SortDirection::Desc,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, utoipa::ToSchema)]
pub enum SortDirection {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

impl SortDirection {
    fn as_str(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

/// Struct used for search functionality when querying various models.
#[derive(Debug, Clone, Validate, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Search<T> {
    #[validate(length(max = 255, message = "Search value too long"))]
    #[validate(custom(function = Self::validate))]
    #[serde(alias = "search.q")]
    pub q: String,

    /// The column to search by when performing the query. This is intended to be an enum of all
    /// the possible search columns for a given query.
    #[serde(alias = "search.column")]
    pub column: T,
}

impl<T> Search<T> {
    fn validate(s: &str) -> Result<(), ValidationError> {
        if s.chars().any(|c| c == ';' || c == '\'' || c.is_control()) {
            return Err(field_err!(
                "search",
                "parameter must be alphanumeric [a-z A-Z 0-9 _-. ]"
            ));
        }
        Ok(())
    }
}

/// Intended to be implemented on enums that serve as search columns.
pub trait ToSearchColumn {
    fn to_search_column(&self) -> &'static str;

    /// Useful when performing joins for prefixing the column with the desired table.
    fn to_search_column_prefixed(&self, prefix: &str) -> String {
        format!("{}.{}", prefix, self.to_search_column())
    }
}

pub trait ToSortColumn {
    fn to_sort_column(&self) -> &'static str;
}

/// Creates an enum with the specified variants that implements [ToSortColumn].
#[macro_export]
macro_rules! sort_column {
    ($name:ident, $($variant:ident => $column:literal),+ $(,)?) => {
        #[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
        #[cfg_attr(test, derive(Clone))]
        #[serde(rename_all = "camelCase")]
        pub enum $name {
            $($variant),+
        }

        impl $crate::model::ToSortColumn for $name {
            fn to_sort_column(&self) -> &'static str {
                match self {
                    $($name::$variant => $column),+
                }
            }
        }
    };
}

/// Creates an enum with the specified variants that implements [ToSearchColumn].
#[macro_export]
macro_rules! search_column {
    ($name:ident, $($variant:ident => $column:literal),+ $(,)?) => {
        #[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
        #[cfg_attr(test, derive(Clone))]
        #[serde(rename_all = "camelCase")]
        pub enum $name {
            $($variant),+
        }

        impl $crate::model::ToSearchColumn for $name {
            fn to_search_column(&self) -> &'static str {
                match self {
                    $($name::$variant => $column),+
                }
            }
        }
    };
}

#[cfg(test)]
mod query {
    use crate::user::model::{UserSearch, UserSort};

    use super::*;

    #[test]
    fn member_list_query() {
        use axum::extract::Query;
        use axum::http::Uri;

        let uri: Uri = "http://localhost/users?page=2&perPage=10".parse().unwrap();
        let Query(parsed) =
            Query::<PaginationSort<UserSort, UserSearch>>::try_from_uri(&uri).unwrap();

        assert_eq!(parsed.limit_offset(), (10, 10));
        assert!(parsed.sort.is_none());
        assert!(parsed.search.is_none());
        parsed.validate().unwrap();

        let sorted: Uri =
            "http://localhost/users?page=1&perPage=10&sort.column=email&sort.direction=asc"
                .parse()
                .unwrap();
        let Query(sorted) =
            Query::<PaginationSort<UserSort, UserSearch>>::try_from_uri(&sorted).unwrap();
        assert_eq!(sorted.sort().0, "email");
        assert_eq!(sorted.sort().1, "ASC");

        let found: Uri = "http://localhost/users?page=1&perPage=10&q=ada&column=username"
            .parse()
            .unwrap();
        let Query(found) =
            Query::<PaginationSort<UserSort, UserSearch>>::try_from_uri(&found).unwrap();
        let search = found.search.as_ref().unwrap();
        assert_eq!(search.q, "ada");
        assert_eq!(search.column.to_search_column(), "username");
        found.validate().unwrap();
    }
}
