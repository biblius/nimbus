#[suitest::suite(files_processing)]
#[suitest::suite_cfg(sequential = true)]
mod processing {
    use super::super::user;
    use crate::{
        db::MetadataRepository,
        test::{self, TestState},
    };
    use image::{GenericImageView, ImageFormat};
    use suitest::before_all;
    use uuid::Uuid;

    #[before_all]
    async fn setup() -> TestState {
        let test_state = TestState::init().await;
        test_state
    }

    async fn wait_until<F, Fut>(timeout: std::time::Duration, mut check: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if check().await {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("condition not met within {timeout:?}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    async fn await_preview(state: &TestState, file_id: Uuid) -> String {
        let metadata = MetadataRepository::new(state.pool.clone());

        wait_until(std::time::Duration::from_secs(5), || {
            let metadata = metadata.clone();
            async move {
                matches!(
                    metadata.get(file_id).await,
                    Ok(Some(row)) if row.preview.is_some()
                )
            }
        })
        .await;

        metadata
            .get(file_id)
            .await
            .unwrap()
            .expect("file_metadata row")
            .preview
            .expect("preview hash")
    }

    fn png_bytes(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbImage::from_fn(48, 32, |_, _| image::Rgb([r, g, b]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    async fn image_upload_creates_64x64_webp_preview(state: TestState) {
        let _ada = user(state, "ada-preview").await;
        let session = test::login(state, "ada-preview", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let before = test::preview_count(state.storage_dir());
        let png = png_bytes(220, 40, 40);
        let entry = test::upload(state, &session.user, root, "photo.png", &png).await;

        let preview_hash = await_preview(state, entry.id).await;
        let path = test::preview_path(state.storage_dir(), &preview_hash);

        assert!(path.is_file(), "preview blob missing at {}", path.display());
        assert_eq!(test::preview_count(state.storage_dir()), before + 1);

        let reader = image::ImageReader::open(&path)
            .unwrap()
            .with_guessed_format()
            .unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::WebP));

        let img = reader.decode().expect("preview must be readable");
        assert_eq!(img.dimensions(), (64, 64));
    }

    #[test]
    async fn identical_images_share_one_preview_blob(state: TestState) {
        let _ada = user(state, "ada-preview-dedup").await;
        let session = test::login(state, "ada-preview-dedup", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let before_previews = test::preview_count(state.storage_dir());
        let before_blobs = test::blob_count(state.storage_dir());

        let png = png_bytes(40, 120, 220);
        let first = test::upload(state, &session.user, root, "a.png", &png).await;
        let second = test::upload(state, &session.user, root, "b.png", &png).await;

        let first_preview = await_preview(state, first.id).await;
        let second_preview = await_preview(state, second.id).await;

        assert_eq!(first_preview, second_preview);
        assert_eq!(first.hash, second.hash);
        assert_eq!(test::preview_count(state.storage_dir()), before_previews + 1);
        assert_eq!(test::blob_count(state.storage_dir()), before_blobs + 1);
    }

    #[test]
    async fn different_images_get_distinct_previews(state: TestState) {
        let _ada = user(state, "ada-preview-distinct").await;
        let session = test::login(state, "ada-preview-distinct", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let before = test::preview_count(state.storage_dir());

        let red = test::upload(
            state,
            &session.user,
            root,
            "red.png",
            &png_bytes(255, 0, 0),
        )
        .await;
        let green = test::upload(
            state,
            &session.user,
            root,
            "green.png",
            &png_bytes(0, 255, 0),
        )
        .await;

        let red_preview = await_preview(state, red.id).await;
        let green_preview = await_preview(state, green.id).await;

        assert_ne!(red_preview, green_preview);
        assert_eq!(test::preview_count(state.storage_dir()), before + 2);
    }

    #[test]
    async fn non_image_upload_skips_preview(state: TestState) {
        let _ada = user(state, "ada-preview-skip").await;
        let session = test::login(state, "ada-preview-skip", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let before = test::preview_count(state.storage_dir());
        let entry = test::upload(state, &session.user, root, "notes.txt", b"hello").await;

        // Processing is async; give the worker a moment, then confirm no metadata.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let metadata = MetadataRepository::new(state.pool.clone())
            .get(entry.id)
            .await
            .unwrap();
        assert!(metadata.is_none());
        assert_eq!(test::preview_count(state.storage_dir()), before);
    }

    #[test]
    async fn directory_list_joins_preview_metadata(state: TestState) {
        let _ada = user(state, "ada-preview-list").await;
        let session = test::login(state, "ada-preview-list", "temporary").await;
        let root = test::private_root(state, &session.user).await;

        let png = png_bytes(10, 20, 30);
        let uploaded = test::upload(state, &session.user, root, "listed.png", &png).await;
        let preview_hash = await_preview(state, uploaded.id).await;

        let listed = state
            .services
            .files
            .list(&session.user, root, None)
            .await
            .unwrap();

        let view = listed
            .iter()
            .find(|row| row.file.id == uploaded.id)
            .expect("uploaded file in listing");

        assert_eq!(view.file.name, "listed.png");
        assert_eq!(
            view.metadata
                .as_ref()
                .and_then(|meta| meta.preview.as_deref()),
            Some(preview_hash.as_str())
        );
    }
}
