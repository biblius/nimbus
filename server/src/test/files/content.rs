#[suitest::suite(files_content)]
#[suitest::suite_cfg(sequential = true)]
mod content {
    use super::super::{ctx, upload_in, user};
    use crate::{
        auth::session::Session,
        db::FileRepository,
        error::AppError,
        file::{
            model::{CreateDirectory, DownloadFiles, PreviewSize},
            service::DownloadBody,
        },
        test::{self, TestState},
    };
    use image::{GenericImageView, ImageFormat};
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
        let username = format!("content-{}", Uuid::new_v4());
        user(state, &username).await;
        let session = test::login(state, &username, "temporary").await;
        let root = test::private_root(state, &session.user).await;
        (session, root)
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

    async fn await_previews(state: &TestState, root_id: Uuid, file_id: Uuid) -> (String, String) {
        let files = FileRepository::new(state.pool.clone());

        wait_until(std::time::Duration::from_secs(5), || {
            let files = files.clone();
            async move {
                matches!(
                    files.get_live(root_id, file_id).await,
                    Ok(Some(row))
                        if row.metadata.preview_64.is_some() && row.metadata.preview_256.is_some()
                )
            }
        })
        .await;

        let meta = files
            .get_live(root_id, file_id)
            .await
            .unwrap()
            .expect("file row")
            .metadata;

        (
            meta.preview_64.expect("preview_64 hash"),
            meta.preview_256.expect("preview_256 hash"),
        )
    }

    fn png_bytes(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbImage::from_fn(48, 32, |_, _| image::Rgb([r, g, b]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn mp4_bytes() -> Vec<u8> {
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=320x240:d=2",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                "mpeg4",
                "-movflags",
                "frag_keyframe+empty_moov",
                "-f",
                "mp4",
                "pipe:1",
            ])
            .output()
            .expect("spawn ffmpeg to synthesize test video");

        assert!(
            output.status.success(),
            "ffmpeg failed to synthesize test video: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty(), "synthesized mp4 was empty");
        output.stdout
    }

    fn assert_webp_size(path: &std::path::Path, expected: (u32, u32)) {
        assert!(path.is_file(), "preview blob missing at {}", path.display());

        let reader = image::ImageReader::open(path)
            .unwrap()
            .with_guessed_format()
            .unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::WebP));

        let img = reader.decode().expect("preview must be readable");
        assert_eq!(img.dimensions(), expected);
    }

    /// Missing id independently yields ResourceMissing on stream.
    #[test]
    async fn stream_missing_file(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .content_for_stream(
                &ctx(state, &session.user, *root).await,
                Uuid::new_v4(),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::ResourceMissing(_)));
    }

    /// Directory independently rejects stream with Unprocessable.
    #[test]
    async fn stream_rejects_directory(state: TestState, session: Session, root: Uuid) {
        let folder = state
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

        let err = state
            .services
            .files
            .content_for_stream(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unprocessable(_)));
    }

    /// Live file stream returns display name, blob path, and mime.
    #[test]
    async fn stream_returns_file_path(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"hello").await;

        let (name, path, mime) = state
            .services
            .files
            .content_for_stream(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();

        assert_eq!(name, "notes.txt");
        assert!(path.is_file());
        assert_eq!(mime, "text/plain; charset=utf-8");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"hello");
    }

    /// Single-file download independently returns DownloadBody::File.
    #[test]
    async fn download_file_body(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"payload").await;

        let (name, body) = state
            .services
            .files
            .content_for_download(&ctx(state, &session.user, *root).await, file.id)
            .await
            .unwrap();

        assert_eq!(name, "notes.txt");
        let DownloadBody::File { path, mime } = body else {
            panic!("expected file download body");
        };
        assert_eq!(mime, "text/plain; charset=utf-8");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"payload");
    }

    /// Directory download independently zips descendants with a .zip name.
    #[test]
    async fn download_directory_zip(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "pack".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        upload_in(
            state,
            &session.user,
            *root,
            Some(folder.id),
            "inside.txt",
            b"in",
        )
        .await;

        let (name, body) = state
            .services
            .files
            .content_for_download(&ctx(state, &session.user, *root).await, folder.id)
            .await
            .unwrap();

        assert_eq!(name, "pack.zip");
        assert!(matches!(body, DownloadBody::Zip { .. }));
    }

    /// Root download independently streams a zip of top-level entries.
    #[test]
    async fn root_download_zip(state: TestState, session: Session, root: Uuid) {
        test::upload(state, &session.user, *root, "one.txt", b"1").await;

        let (name, body) = state
            .services
            .files
            .content_for_root_download(&ctx(state, &session.user, *root).await)
            .await
            .unwrap();

        assert!(name.ends_with(".zip"));
        assert!(matches!(body, DownloadBody::Zip { .. }));
    }

    /// Empty ids independently fails validify on download_dir.
    #[test]
    async fn download_dir_rejects_empty_ids(state: TestState, session: Session, root: Uuid) {
        let err = state
            .services
            .files
            .download_dir(
                &ctx(state, &session.user, *root).await,
                DownloadFiles { ids: vec![] },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Directory independently rejects preview with Unprocessable.
    #[test]
    async fn preview_rejects_directory(state: TestState, session: Session, root: Uuid) {
        let folder = state
            .services
            .files
            .create_directory(
                &ctx(state, &session.user, *root).await,
                CreateDirectory {
                    parent: None,
                    name: "photos".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let result = state
            .services
            .files
            .thumbnail(
                &ctx(state, &session.user, *root).await,
                folder.id,
                PreviewSize::Icon,
            )
            .await;

        assert!(matches!(result, Err(AppError::Unprocessable(_))));
    }

    /// Plain text without preview metadata independently yields ResourceMissing.
    #[test]
    async fn preview_missing_without_metadata(state: TestState, session: Session, root: Uuid) {
        let file = test::upload(state, &session.user, *root, "notes.txt", b"hello").await;

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let result = state
            .services
            .files
            .thumbnail(
                &ctx(state, &session.user, *root).await,
                file.id,
                PreviewSize::Icon,
            )
            .await;

        assert!(matches!(result, Err(AppError::ResourceMissing(_))));
    }

    #[test]
    async fn image_upload_creates_64_and_256_webp_previews(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let before = test::preview_count(state.storage_dir());
        let png = png_bytes(220, 40, 40);
        let entry = test::upload(state, &session.user, *root, "photo.png", &png).await;

        let (icon_hash, tile_hash) = await_previews(state, *root, entry.id).await;
        assert_ne!(icon_hash, tile_hash);

        let icon_path = test::preview_path(state.storage_dir(), &icon_hash);
        let tile_path = test::preview_path(state.storage_dir(), &tile_hash);

        assert_webp_size(&icon_path, (64, 64));
        assert_webp_size(&tile_path, (256, 256));
        assert_eq!(test::preview_count(state.storage_dir()), before + 2);
    }

    #[test]
    async fn video_upload_creates_64_and_256_webp_previews(
        state: TestState,
        session: Session,
        root: Uuid,
    ) {
        let before = test::preview_count(state.storage_dir());
        let mp4 = mp4_bytes();
        let entry = test::upload(state, &session.user, *root, "clip.mp4", &mp4).await;

        let (icon_hash, tile_hash) = await_previews(state, *root, entry.id).await;
        assert_ne!(icon_hash, tile_hash);

        assert_webp_size(
            &test::preview_path(state.storage_dir(), &icon_hash),
            (64, 64),
        );
        assert_webp_size(
            &test::preview_path(state.storage_dir(), &tile_hash),
            (256, 256),
        );
        assert_eq!(test::preview_count(state.storage_dir()), before + 2);

        let icon = state
            .services
            .files
            .thumbnail(
                &ctx(state, &session.user, *root).await,
                entry.id,
                PreviewSize::Icon,
            )
            .await
            .unwrap();
        assert_eq!(icon.mime, "image/webp");
        assert!(icon.len > 0);

        let tile = state
            .services
            .files
            .thumbnail(
                &ctx(state, &session.user, *root).await,
                entry.id,
                PreviewSize::Tile,
            )
            .await
            .unwrap();
        assert_eq!(tile.mime, "image/webp");
        assert!(tile.len > 0);
    }
}
