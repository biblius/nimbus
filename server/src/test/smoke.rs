#[suitest::suite(smoke)]
#[suitest::suite_cfg(sequential = true)]
mod smoke {
    use crate::test::TestState;
    use suitest::before_all;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    #[test]
    async fn database_accepts_queries(state: TestState) {
        sqlx::query("SELECT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    }

    #[test]
    async fn cache_roundtrip(state: TestState) {
        state.cache.set_ex("k", "v", 60).await.unwrap();
        assert_eq!(
            state.cache.get_del("k").await.unwrap().as_deref(),
            Some("v")
        );
        assert_eq!(state.cache.get_del("k").await.unwrap(), None);
    }
}
