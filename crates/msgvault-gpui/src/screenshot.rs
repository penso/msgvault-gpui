//! `--screenshot OUT.png`: renders the real window headlessly through Metal
//! against a generated demo archive. Used to review the UI without a display
//! session, and to produce the README images. Opt-in: `--features screenshot`.

use {
    crate::app::{Launch, MailApp},
    gpui_kit::{
        AppContext as _, HeadlessAppContext,
        component::{Root, ThemeMode},
        px, size,
        test::TestWindowExt as _,
    },
    msgvault_core::BackendChoice,
    std::{path::PathBuf, sync::Arc},
};

pub(crate) struct Shot {
    pub output: PathBuf,
    pub dark: bool,
    pub width: f32,
    pub height: f32,
    /// Move the selection down this many messages before capturing.
    pub select: usize,
    pub mailbox: Option<String>,
    pub search: Option<String>,
}

pub(crate) fn run(shot: Shot) -> anyhow::Result<()> {
    let home = std::env::temp_dir().join("msgvault-gpui-screenshot");
    msgvault_core::fixture::generate(&home, chrono::Utc::now())?;

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(crate::assets::AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::configure(cx);
    });
    let launch = Launch {
        home,
        choice: BackendChoice::Sqlite,
        demo: false,
    };
    let mut view = None;
    let handle = cx.open_window(size(px(shot.width), px(shot.height)), |window, cx| {
        let app = cx.new(|cx| MailApp::new(launch, window, cx));
        view = Some(app.clone());
        cx.new(|cx| Root::new(app, window, cx))
    })?;
    let view = view.ok_or_else(|| anyhow::anyhow!("window did not build its view"))?;
    let settle = |cx: &mut HeadlessAppContext| -> anyhow::Result<()> {
        for _ in 0..4 {
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))?;
        }
        Ok(())
    };
    settle(&mut cx)?;
    let mode = if shot.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |app, cx| {
            app.set_theme_mode(mode, window, cx);
            app.set_display_location("~/.msgvault/msgvault.db", cx);
            if let Some(mailbox) = &shot.mailbox {
                app.show_mailbox_named(mailbox, window, cx);
            }
            if let Some(search) = &shot.search {
                app.search_for(search, window, cx);
            }
        });
    })?;
    settle(&mut cx)?;
    for _ in 0..shot.select {
        cx.update_window(handle.into(), |_, _, cx| {
            view.update(cx, |app, cx| app.step(1, cx))
        })?;
        settle(&mut cx)?;
    }
    let image = cx.capture_screenshot(handle.into())?;
    image.save(&shot.output)?;
    println!("Wrote {}", shot.output.display());
    Ok(())
}
