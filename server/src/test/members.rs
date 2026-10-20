#[suitest::suite(members_service)]
#[suitest::suite_cfg(sequential = true)]
mod members_service {
    use crate::{
        error::AppError,
        model::{Pagination, PaginationSort, Search, Sort, SortDirection},
        test::{self, TestState},
        user::model::{CreateSharedFolder, CreateUser, Role, UserSearch, UserSort},
    };
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn pages_follow_the_sort_and_keep_the_full_total(state: TestState) {
        let ada = user(state, "sort-a", None).await;
        let bob = user(state, "sort-b", None).await;
        let cara = user(state, "sort-c", None).await;
        let shared = shared_folder(state, "sorted", &[cara, bob, ada]).await;
        let session = test::login(state, "sort-a", "temporary").await;

        let ascending = Sort {
            column: UserSort::Username,
            direction: SortDirection::Asc,
        };

        let first = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                page(1, 2, Some(ascending.clone()), None),
            )
            .await
            .unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(usernames(&first), ["sort-a", "sort-b"]);

        let second = state
            .services
            .users
            .list_members(&session.user, shared, page(2, 2, Some(ascending), None))
            .await
            .unwrap();
        assert_eq!(second.total, 3);
        assert_eq!(usernames(&second), ["sort-c"]);

        let descending = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                page(
                    1,
                    2,
                    Some(Sort {
                        column: UserSort::Username,
                        direction: SortDirection::Desc,
                    }),
                    None,
                ),
            )
            .await
            .unwrap();
        assert_eq!(descending.total, 3);
        assert_eq!(usernames(&descending), ["sort-c", "sort-b"]);

        let past_the_end = state
            .services
            .users
            .list_members(&session.user, shared, page(3, 2, None, None))
            .await
            .unwrap();
        assert_eq!(past_the_end.total, 3);
        assert!(past_the_end.items.is_empty());
    }

    #[test]
    async fn omitted_pagination_returns_the_first_ten(state: TestState) {
        let mut members = Vec::with_capacity(11);
        for index in 1..=11 {
            let created = user(state, &format!("limit-{index:02}"), None).await;
            members.push(created);
        }
        let shared = shared_folder(state, "limited", &members).await;
        let session = test::login(state, "limit-01", "temporary").await;

        let listed = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                PaginationSort {
                    pagination: None,
                    sort: Some(Sort {
                        column: UserSort::Username,
                        direction: SortDirection::Asc,
                    }),
                    search: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(listed.total, 11);
        assert_eq!(listed.items.len(), 10);
        assert_eq!(listed.items[0].username, "limit-01");
        assert_eq!(listed.items[9].username, "limit-10");
    }

    #[test]
    async fn search_narrows_the_page(state: TestState) {
        let amy = user(state, "find-amy", None).await;
        let bea = user(state, "find-bea", Some("bea@example.com".into())).await;
        let cy = user(state, "find-cy", None).await;
        let shared = shared_folder(state, "searchable", &[amy, bea, cy]).await;
        let session = test::login(state, "find-amy", "temporary").await;

        let by_username = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                page(
                    1,
                    10,
                    None,
                    Some(Search {
                        q: "find-b".into(),
                        column: UserSearch::Username,
                    }),
                ),
            )
            .await
            .unwrap();
        assert_eq!(by_username.total, 1);
        assert_eq!(usernames(&by_username), ["find-bea"]);

        let by_email = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                page(
                    1,
                    10,
                    None,
                    Some(Search {
                        q: "bea@example".into(),
                        column: UserSearch::Email,
                    }),
                ),
            )
            .await
            .unwrap();
        assert_eq!(by_email.total, 1);
        assert_eq!(usernames(&by_email), ["find-bea"]);

        let none = state
            .services
            .users
            .list_members(
                &session.user,
                shared,
                page(
                    1,
                    10,
                    None,
                    Some(Search {
                        q: "missing".into(),
                        column: UserSearch::Username,
                    }),
                ),
            )
            .await
            .unwrap();
        assert_eq!(none.total, 0);
        assert!(none.items.is_empty());
    }

    #[test]
    async fn members_and_admins_can_list_strangers_cannot(state: TestState) {
        let ada = user(state, "access-ada", None).await;
        let _bob = user(state, "access-bob", None).await;
        let shared = shared_folder(state, "guarded", &[ada]).await;

        let member = test::login(state, "access-ada", "temporary").await;
        let listed = state
            .services
            .users
            .list_members(&member.user, shared, PaginationSort::default())
            .await
            .unwrap();
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, ada);

        let all = state
            .services
            .users
            .list_all_members(&member.user, shared)
            .await
            .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, ada);

        let stranger = test::login(state, "access-bob", "temporary").await;
        let forbidden = state
            .services
            .users
            .list_members(&stranger.user, shared, PaginationSort::default())
            .await;
        assert!(matches!(forbidden, Err(AppError::Forbidden(_))));

        let forbidden_all = state
            .services
            .users
            .list_all_members(&stranger.user, shared)
            .await;
        assert!(matches!(forbidden_all, Err(AppError::Forbidden(_))));

        let admin = test::login(state, "admin", "admin").await;
        let as_admin = state
            .services
            .users
            .list_members(&admin.user, shared, PaginationSort::default())
            .await
            .unwrap();
        assert_eq!(as_admin.total, 1);
        assert_eq!(as_admin.items[0].id, ada);
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

    fn usernames(listed: &crate::model::List<crate::user::model::Member>) -> Vec<&str> {
        listed
            .items
            .iter()
            .map(|member| member.username.as_str())
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

    async fn shared_folder(state: &TestState, name: &str, members: &[Uuid]) -> Uuid {
        state
            .services
            .users
            .create_shared(CreateSharedFolder {
                name: name.into(),
                quota_bytes: 0,
                members: members.iter().copied().map(Into::into).collect(),
            })
            .await
            .unwrap()
            .id
    }
}
