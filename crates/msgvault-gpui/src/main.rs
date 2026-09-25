//! A native mail-client viewer for msgvault archives.

mod actions;
mod app;
mod assets;
mod format;
#[cfg(feature = "screenshot")]
mod screenshot;
mod theme;

use {
    app::{Launch, MailApp},
    gpui_kit::{
        AppContext as _, Bounds, WindowBounds, WindowOptions,
        component::{Root, TitleBar},
        px, size,
    },
    msgvault_core::{ArchiveLocation, BackendChoice},
    std::{path::PathBuf, process::ExitCode},
};

const USAGE: &str = "\
msgvault-gpui: browse a msgvault archive

Usage: msgvault-gpui [OPTIONS]

Options:
  --home DIR       msgvault home (default: $MSGVAULT_HOME or ~/.msgvault)
  --sqlite         read msgvault.db directly, never the daemon
  --daemon         require a running `msgvault serve` daemon
  --url URL        connect to the daemon at URL (e.g. http://nas.local:8080)
  --api-key KEY    API key for --url
  --demo           open a generated demo archive
  -h, --help       show this help

Screenshot mode (built with --features screenshot):
  --screenshot OUT.png [--dark] [--size WxH] [--select N]
                   [--mailbox NAME] [--search QUERY]";

enum Mode {
    Window(Launch),
    #[cfg(feature = "screenshot")]
    Screenshot(screenshot::Shot),
}

fn parse_args() -> Result<Option<Mode>, String> {
    let mut home: Option<PathBuf> = None;
    let mut choice = BackendChoice::Auto;
    let mut url: Option<String> = None;
    let mut api_key: Option<String> = None;
    let mut demo = false;
    let mut shot: Option<PathBuf> = None;
    let (mut dark, mut width, mut height, mut select) = (false, 1440.0_f32, 900.0_f32, 0_usize);
    let (mut mailbox, mut search): (Option<String>, Option<String>) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--home" => home = Some(PathBuf::from(value("--home")?)),
            "--sqlite" => choice = BackendChoice::Sqlite,
            "--daemon" => choice = BackendChoice::Daemon,
            "--url" => url = Some(value("--url")?),
            "--api-key" => api_key = Some(value("--api-key")?),
            "--demo" => demo = true,
            "--screenshot" => shot = Some(PathBuf::from(value("--screenshot")?)),
            "--dark" => dark = true,
            "--select" => {
                select = value("--select")?
                    .parse()
                    .map_err(|_| "--select needs a number")?
            },
            "--mailbox" => mailbox = Some(value("--mailbox")?),
            "--search" => search = Some(value("--search")?),
            "--size" => {
                let raw = value("--size")?;
                let (w, h) = raw.split_once('x').ok_or("--size needs WxH")?;
                width = w.parse().map_err(|_| "--size needs WxH")?;
                height = h.parse().map_err(|_| "--size needs WxH")?;
            },
            "-h" | "--help" => return Ok(None),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if let Some(url) = url {
        choice = BackendChoice::Url { url, api_key };
    }
    if let Some(output) = shot {
        #[cfg(feature = "screenshot")]
        return Ok(Some(Mode::Screenshot(screenshot::Shot {
            output,
            dark,
            width,
            height,
            select,
            mailbox,
            search,
        })));
        #[cfg(not(feature = "screenshot"))]
        {
            let _ = (output, dark, width, height, select, mailbox, search);
            return Err(
                "this build has no screenshot support; rebuild with --features screenshot".into(),
            );
        }
    }
    let _ = (dark, width, height, select, &mailbox, &search);
    let home = home.unwrap_or_else(ArchiveLocation::default_home);
    Ok(Some(Mode::Window(Launch { home, choice, demo })))
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("error,msgvault_core=info,msgvault_gpui=info")
            }),
        )
        .with_writer(std::io::stderr)
        .init();
    let launch = match parse_args() {
        Ok(Some(Mode::Window(launch))) => launch,
        #[cfg(feature = "screenshot")]
        Ok(Some(Mode::Screenshot(shot))) => {
            return match screenshot::run(shot) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("msgvault-gpui: screenshot failed: {error:#}");
                    ExitCode::FAILURE
                },
            };
        },
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        },
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        },
    };

    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::configure(cx);

            actions::bind_keys(cx);
            cx.set_menus(actions::menus());
            cx.on_action(|_: &actions::Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(1380.), px(860.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(560.))),
                app_id: Some("dev.msgvault.gpui".into()),
                ..TitleBar::window_options()
            };
            let opened = cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| MailApp::new(launch, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("msgvault-gpui: unable to open a window: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
