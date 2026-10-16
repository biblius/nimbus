#[suitest::suite(users_service)]
#[suitest::suite_cfg(sequential = true)]
mod users_service {
    use crate::{
        model::{Pagination, PaginationSort, Search, Sort, SortDirection},
        test::TestState,
        user::model::{CreateUser, Role, UserSearch, UserSort},
    };
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn pages_follow_the_sort_and_search_narrows_them(state: TestState) {
        user(state, "list-amy", Some("amy@example.com".into())).await;
        user(state, "list-bea", Some("bea@example.com".into())).await;
        user(state, "list-cy", Some("cy@example.com".into())).await;

        let ascending = Sort {
            column: UserSort::Username,
            direction: SortDirection::Asc,
        };
        let narrow = Some(Search {
            q: "list-".into(),
            column: UserSearch::Username,
        });

        let first = state
            .services
            .users
            .list_users(page(1, 2, Some(ascending.clone()), narrow.clone()))
            .await
            .unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(usernames(&first), ["list-amy", "list-bea"]);

        let second = state
            .services
            .users
            .list_users(page(2, 2, Some(ascending), narrow.clone()))
            .await
            .unwrap();
        assert_eq!(second.total, 3);
        assert_eq!(usernames(&second), ["list-cy"]);

        let by_email = state
            .services
            .users
            .list_users(page(
                1,
                10,
                Some(Sort {
                    column: UserSort::Email,
                    direction: SortDirection::Asc,
                }),
                Some(Search {
                    q: "bea@".into(),
                    column: UserSearch::Email,
                }),
            ))
            .await
            .unwrap();
        assert_eq!(by_email.total, 1);
        assert_eq!(usernames(&by_email), ["list-bea"]);
    }

    fn page(
        page: usize,
        per_page: usize,
        sort: Option<Sort<UserSort>>,
        search: Option<Search<UserSearch>>,
    ) -> PaginationSort<UserSort, UserSearch> {
        PaginationSort {
            pagination: Some(Pagination::new(page, per_page)),
            sort,
            search,
        }
    }

    fn usernames(listed: &crate::model::List<crate::user::model::UserView>) -> Vec<&str> {
        listed
            .items
            .iter()
            .map(|user| user.username.as_str())
            .collect()
    }

    async fn user(state: &TestState, username: &str, email: Option<String>) -> Uuid {
        state
            .services
            .users
            .create_user(CreateUser {
                username: username.into(),
                password: "temporary".into(),
                email,
                role: Role::User,
            })
            .await
            .unwrap()
            .id
    }
}
