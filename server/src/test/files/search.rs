#[suitest::suite(files_search)]
#[suitest::suite_cfg(sequential = true)]
mod search {
    use super::super::{ctx, shared_root, upload_in, user};
    use crate::{
        auth::session::Session,
        file::model::{CreateDirectory, FileView, RenameFile, TrashFiles},
        test::{self, TestState},
    };
    use suitest::{before_all, before_each};
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    /// Private workspace: logged-in user and their root.
    #[before_each]
    async fn workspace(state: TestState) -> (Session, Uuid) {
        let username = format!("search-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
    }

    fn names(hits: &[FileView]) -> Vec<&str> {
        hits.iter().map(|hit| hit.file.name.as_str()).collect()
    }

    async fn search(
        state: &TestState,
        session: &Session,
        root: Uuid,
        q: &str,
    ) -> Vec<FileView> {
        state
            .services
            .files
            .search(&ctx(state, &session.user, root).await, q)
            .await
            .unwrap()
    }

    /// Empty query independently returns no rows (trim held empty).
    #[test]
    async fn empty_query_returns_no_hits(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let hits = search(state, &session, *root, "").await;
        assert!(hits.is_empty());
    }

    /// Whitespace-only query independently returns no rows (same empty path as "").
    #[test]
    async fn whitespace_query_returns_no_hits(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let hits = search(state, &session, *root, "   \t  ").await;
        assert!(hits.is_empty());
    }

    /// Non-matching query independently yields an empty result (files held present).
    #[test]
    async fn unmatched_query_returns_no_hits(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "notes.txt", b"notes").await;

        let hits = search(state, &session, *root, "missing").await;
        assert!(hits.is_empty());
    }

    /// Substring match independently includes the file (root held current).
    #[test]
    async fn substring_match_finds_file(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "quarterly-report.pdf", b"pdf").await;

        let hits = search(state, &session, *root, "report").await;
        assert_eq!(names(&hits), ["quarterly-report.pdf"]);
        assert_eq!(hits[0].file.id, file.id);
    }

    /// Directory names independently participate in search (files held present).
    #[test]
    async fn substring_match_finds_directory(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "Reports".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        test::upload(state, &session.user, *root, "other.txt", b"x").await;

        let hits = search(state, &session, *root, "Reports").await;
        assert_eq!(names(&hits), ["Reports"]);
        assert_eq!(hits[0].file.id, folder.id);
        assert_eq!(hits[0].file.kind, crate::file::model::FileKind::Directory);
    }

    /// Case differs independently still matches (substring held true).
    #[test]
    async fn match_is_case_insensitive(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "Report.PDF", b"pdf").await;

        let hits = search(state, &session, *root, "report").await;
        assert_eq!(names(&hits), ["Report.PDF"]);
    }

    /// Nested path independently does not block name match (parent held set).
    #[test]
    async fn finds_nested_entries_by_name(state: TestState, session: Session, root: Uuid) {
        let docs = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "docs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let nested = upload_in(
            state,
            &session.user,
            *root,
            Some(docs.id),
            "deep-notes.txt",
            b"nested",
        )
        .await;

        let hits = search(state, &session, *root, "deep-notes").await;
        assert_eq!(names(&hits), ["deep-notes.txt"]);
        assert_eq!(hits[0].file.id, nested.id);
        assert_eq!(hits[0].file.parent, Some(docs.id));
    }

    /// Other root independently excludes hits (name match held true on the other root).
    #[test]
    async fn search_is_scoped_to_the_current_root(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "only-here.txt", b"a").await;

        let other_name = format!("search-other-{}", Uuid::new_v4());
        user(state, &other_name).await;
        let other_session = test::login(state, &other_name, "temporary").await;
        let other_root = test::private_root(state, &other_session.user).await;
        test::upload(
            state,
            &other_session.user,
            other_root,
            "only-here.txt",
            b"b",
        )
        .await;

        let hits = search(state, &session, *root, "only-here").await;
        assert_eq!(names(&hits), ["only-here.txt"]);
        assert_eq!(hits[0].file.root_id, *root);

        let other_hits = search(state, &other_session, other_root, "only-here").await;
        assert_eq!(names(&other_hits), ["only-here.txt"]);
        assert_eq!(other_hits[0].file.root_id, other_root);
        assert_ne!(hits[0].file.id, other_hits[0].file.id);
    }

    /// Shared-root member independently searches that root (private root held separate).
    #[test]
    async fn member_can_search_shared_root(state: TestState) {
        let ada = user(state, &format!("search-share-a-{}", Uuid::new_v4())).await;
        let bob = user(state, &format!("search-share-b-{}", Uuid::new_v4())).await;
        let ada_session = test::login(state, &ada.username, "temporary").await;
        let bob_session = test::login(state, &bob.username, "temporary").await;

        let shared = shared_root(state, "search-shared", &[ada.id, bob.id]).await;
        upload_in(
            state,
            &ada_session.user,
            shared,
            None,
            "shared-memo.txt",
            b"memo",
        )
        .await;

        let hits = search(state, &bob_session, shared, "shared-memo").await;
        assert_eq!(names(&hits), ["shared-memo.txt"]);
    }

    /// Prefix match independently ranks above mid-string match (both held matching).
    #[test]
    async fn prefix_ranks_before_mid_substring(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "my-report.txt", b"a").await;
        test::upload(state, &session.user, *root, "report.txt", b"b").await;

        let hits = search(state, &session, *root, "report").await;
        assert_eq!(names(&hits), ["report.txt", "my-report.txt"]);
    }

    /// Shorter name independently ranks above longer when prefix-ness is equal.
    #[test]
    async fn shorter_name_ranks_before_longer_prefix_peer(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        test::upload(state, &session.user, *root, "repository.txt", b"a").await;
        test::upload(state, &session.user, *root, "rep.txt", b"b").await;

        let hits = search(state, &session, *root, "rep").await;
        assert_eq!(names(&hits), ["rep.txt", "repository.txt"]);
    }

    /// Literal `%` in the query independently matches only a literal `%` in the name.
    ///
    /// Without LIKE escaping, `a%b` would also match `axxb` (`%` = any sequence).
    #[test]
    async fn escaped_percent_is_literal(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "a%b.txt", b"lit").await;
        test::upload(state, &session.user, *root, "axxb.txt", b"wild").await;

        let hits = search(state, &session, *root, "a%b").await;
        assert_eq!(names(&hits), ["a%b.txt"]);
    }

    /// Literal `_` in the query independently matches only a literal `_` in the name.
    ///
    /// Without LIKE escaping, `a_b` would also match `axb` (`_` = any one char).
    #[test]
    async fn escaped_underscore_is_literal(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "a_b.txt", b"lit").await;
        test::upload(state, &session.user, *root, "axb.txt", b"wild").await;

        let hits = search(state, &session, *root, "a_b").await;
        assert_eq!(names(&hits), ["a_b.txt"]);
    }

    /// Rename independently moves the hit from the old name to the new name.
    #[test]
    async fn rename_updates_searchable_name(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "before.txt", b"x").await;

        assert_eq!(names(&search(state, &session, *root, "before").await), ["before.txt"]);
        assert!(search(state, &session, *root, "after").await.is_empty());

        state
            .services
            .files
            .rename(
                &ctx(state, &session.user, *root).await,
                file.id,
                RenameFile {
                    name: "after.txt".into(),
                },
            )
            .await
            .unwrap();

        assert!(search(state, &session, *root, "before").await.is_empty());
        assert_eq!(names(&search(state, &session, *root, "after").await), ["after.txt"]);
    }

    /// Trashing independently removes the live name from search results.
    #[test]
    async fn trashed_file_is_not_searchable(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "doomed.txt", b"x").await;

        state
            .services
            .files
            .delete_many(
                &ctx(state, &session.user, *root).await,
                TrashFiles {
                    ids: vec![file.id],
                },
            )
            .await
            .unwrap();

        let hits = search(state, &session, *root, "doomed").await;
        assert!(hits.is_empty());
    }

    /// Leading/trailing query whitespace independently does not change matching
    /// (inner trim held applied before search).
    #[test]
    async fn query_is_trimmed_before_match(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "notes.txt", b"n").await;

        let hits = search(state, &session, *root, "  notes  ").await;
        assert_eq!(names(&hits), ["notes.txt"]);
    }

    /// File rows independently are omitted from the directories map (dirs held present).
    #[test]
    async fn directories_map_excludes_files(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "only-dir".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let file = test::upload(state, &session.user, *root, "skip-me.txt", b"x").await;

        let dirs = state
            .services
            .files
            .directories(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert_eq!(dirs.len(), 1);
        assert!(dirs.contains_key(&folder.id));
        assert!(!dirs.contains_key(&file.id));
        assert_eq!(dirs[&folder.id].name, "only-dir");
        assert_eq!(
            dirs[&folder.id].kind,
            crate::file::model::FileKind::Directory
        );
    }

    /// Nested directories independently appear in the map keyed by id.
    #[test]
    async fn directories_map_includes_nested_dirs(state: TestState, session: Session, root: Uuid) {
        let parent = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "parent".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let child = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: Some(parent.id),
                    name: "child".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let dirs = state
            .services
            .files
            .directories(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[&parent.id].parent, None);
        assert_eq!(dirs[&child.id].parent, Some(parent.id));
    }

    /// Empty root independently yields an empty directories map.
    #[test]
    async fn directories_map_empty_when_no_dirs(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "lonely.txt", b"x").await;

        let dirs = state
            .services
            .files
            .directories(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert!(dirs.is_empty());
    }

    /// Other root's directories independently stay out of the map.
    #[test]
    async fn directories_map_is_scoped_to_root(state: TestState, session: Session, root: Uuid) {
        state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "mine".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let other_name = format!("search-dirs-{}", Uuid::new_v4());
        user(state, &other_name).await;
        let other_session = test::login(state, &other_name, "temporary").await;
        let other_root = test::private_root(state, &other_session.user).await;
        state
            .services
            .files
            .create_directory(
                &ctx(state, &other_session.user, other_root).await,
                CreateDirectory {
                    parent: None,
                    name: "theirs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let mine = state
            .services
            .files
            .directories(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();
        assert_eq!(mine.len(), 1);
        assert!(mine.values().any(|entry| entry.name == "mine"));
        assert!(!mine.values().any(|entry| entry.name == "theirs"));
    }

    /// Short / prefix-like query independently matches via substring (`ILIKE` / `LIKE`)
    /// even when trigram similarity alone would be weak.
    #[test]
    async fn short_query_matches_substring(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "report.pdf", b"pdf").await;

        let hits = search(state, &session, *root, "rep").await;
        assert_eq!(names(&hits), ["report.pdf"]);
    }

    /// Typo that is not a substring: Postgres `pg_trgm` independently matches;
    /// SQLite has no trigrams so the same query independently yields no hits.
    ///
    /// `raport` is not contained in `report.pdf`; only similarity can hit.
    #[test]
    async fn typo_match_depends_on_trigram_backend(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        test::upload(state, &session.user, *root, "report.pdf", b"pdf").await;

        let hits = search(state, &session, *root, "raport").await;

        #[cfg(feature = "postgres")]
        assert_eq!(names(&hits), ["report.pdf"]);

        #[cfg(feature = "sqlite")]
        assert!(
            hits.is_empty(),
            "sqlite substring search must not treat typos as hits: {hits:?}"
        );
    }
}
