//! The main window: state, background loading, and the three-pane layout.

mod list;
mod reader;
mod sidebar;
mod titlebar;
mod welcome;

use {
    crate::{actions::*, format},
    gpui_kit::{
        component::{
            ActiveTheme, Root, ThemeMode, WindowExt as _, h_resizable,
            input::{InputEvent, InputState},
            notification::Notification,
            resizable_panel,
        },
        *,
    },
    msgvault_core::{
        self as mv, Account, AccountId, ArchiveLocation, Backend, BackendChoice, BackendInfo,
        ConversationId, Label, Mailbox, MailboxCounts, MessageId, MessageQuery, MessageSummary,
        Stats, Thread,
    },
    std::{
        collections::HashSet,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
};

const PAGE_SIZE: u64 = 100;
/// Start fetching the next page when this close to the end of the list.
const LOAD_AHEAD: usize = 25;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);

/// How the window was asked to find its archive.
#[derive(Clone, Debug)]
pub(crate) struct Launch {
    pub home: PathBuf,
    pub choice: BackendChoice,
    pub demo: bool,
}

/// Where the archive connection stands.
enum Connection {
    Connecting,
    /// No archive at the expected location.
    Missing {
        path: PathBuf,
    },
    Failed(SharedString),
    Ready,
}

enum ReaderState {
    Empty,
    Loading,
    Ready(Arc<Thread>),
    Failed(SharedString),
}

pub(crate) struct MailApp {
    launch: Launch,
    connection: Connection,
    backend: Option<Arc<dyn Backend>>,
    info: Option<BackendInfo>,

    accounts: Vec<Account>,
    labels: Vec<Label>,
    counts: MailboxCounts,
    stats: Stats,

    account: Option<AccountId>,
    mailbox: Mailbox,
    search: Entity<InputState>,
    query: Option<String>,

    messages: Vec<MessageSummary>,
    has_more: bool,
    list_loading: bool,
    list_error: Option<SharedString>,
    selected: Option<MessageId>,
    reader: ReaderState,
    /// Conversation messages shown in full; the rest collapse to one line.
    expanded: HashSet<MessageId>,
    rich_text: bool,
    /// Track the OS appearance until the user picks a mode.
    follow_system: bool,
    /// Conversations already listed; the list shows one row per conversation.
    listed: HashSet<ConversationId>,
    /// Messages fetched for the current list, the offset of the next page.
    fetched: u64,

    list_scroll: UniformListScrollHandle,
    reader_scroll: ScrollHandle,
    focus: FocusHandle,
    list_generation: u64,
    reader_generation: u64,
    meta_generation: u64,
    search_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl MailApp {
    pub(crate) fn new(launch: Launch, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search mail — from:, subject:, label:, has:attachment")
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        crate::theme::apply(None, Some(window), cx);
        let subscriptions = vec![
            cx.subscribe_in(&search, window, Self::on_search_event),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.follow_system {
                    crate::theme::apply(None, Some(window), cx);
                }
            }),
        ];
        let mut this = Self {
            launch,
            connection: Connection::Connecting,
            backend: None,
            info: None,
            accounts: Vec::new(),
            labels: Vec::new(),
            counts: MailboxCounts::default(),
            stats: Stats::default(),
            account: None,
            mailbox: Mailbox::Inbox,
            search,
            query: None,
            messages: Vec::new(),
            has_more: false,
            list_loading: false,
            list_error: None,
            selected: None,
            reader: ReaderState::Empty,
            expanded: HashSet::new(),
            rich_text: true,
            follow_system: true,
            listed: HashSet::new(),
            fetched: 0,
            list_scroll: UniformListScrollHandle::new(),
            reader_scroll: ScrollHandle::new(),
            focus,
            list_generation: 0,
            reader_generation: 0,
            meta_generation: 0,
            search_task: None,
            _subscriptions: subscriptions,
        };
        if this.launch.demo {
            this.open_demo(cx);
        } else {
            this.connect(cx);
        }
        this
    }

    /// Opens the archive off the UI thread: daemon discovery probes the network.
    fn connect(&mut self, cx: &mut Context<Self>) {
        self.connection = Connection::Connecting;
        let launch = self.launch.clone();
        let task = cx.background_executor().spawn(async move {
            let location = ArchiveLocation::load(&launch.home)?;
            mv::open(&location, &launch.choice)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| this.connected(result, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    fn open_demo(&mut self, cx: &mut Context<Self>) {
        self.connection = Connection::Connecting;
        let dir = std::env::temp_dir().join("msgvault-gpui-demo");
        let task = cx.background_executor().spawn(async move {
            mv::fixture::generate(&dir, chrono::Utc::now())?;
            let location = ArchiveLocation::load(&dir)?;
            mv::open(&location, &BackendChoice::Sqlite)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| this.connected(result, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    fn connected(&mut self, result: mv::Result<Arc<dyn Backend>>, cx: &mut Context<Self>) {
        match result {
            Ok(backend) => {
                self.info = Some(backend.describe());
                self.backend = Some(backend);
                self.connection = Connection::Ready;
                self.refresh(cx);
            },
            Err(mv::Error::ArchiveNotFound { path }) => {
                self.connection = Connection::Missing { path };
            },
            Err(error) => {
                tracing::error!(?error, "unable to open archive");
                self.connection = Connection::Failed(error_chain(&error).into());
            },
        }
        cx.notify();
    }

    /// Runs `job` against the backend on a worker and applies the result.
    fn run<T: Send + 'static>(
        &self,
        cx: &mut Context<Self>,
        job: impl FnOnce(&dyn Backend) -> mv::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, mv::Result<T>, &mut Context<Self>) + 'static,
    ) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { job(backend.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_metadata(cx);
        self.reload_list(cx);
    }

    fn load_metadata(&mut self, cx: &mut Context<Self>) {
        self.meta_generation += 1;
        let generation = self.meta_generation;
        let account = self.account;
        self.run(
            cx,
            move |backend| {
                Ok((
                    backend.accounts()?,
                    backend.labels(account)?,
                    backend.mailbox_counts(account)?,
                    backend.stats()?,
                ))
            },
            move |this, result, _| {
                if generation != this.meta_generation {
                    return;
                }
                match result {
                    Ok((accounts, labels, counts, stats)) => {
                        this.accounts = accounts;
                        this.labels = labels;
                        this.counts = counts;
                        this.stats = stats;
                    },
                    Err(error) => {
                        tracing::warn!(?error, "unable to load archive metadata");
                        this.list_error = Some(error_chain(&error).into());
                    },
                }
            },
        );
    }

    fn current_query(&self, offset: u64) -> MessageQuery {
        MessageQuery {
            mailbox: self.mailbox.clone(),
            account: self.account,
            search: self.query.clone(),
            offset,
            limit: PAGE_SIZE,
        }
    }

    fn reload_list(&mut self, cx: &mut Context<Self>) {
        self.list_generation += 1;
        let generation = self.list_generation;
        self.list_loading = true;
        self.list_error = None;
        let query = self.current_query(0);
        self.run(
            cx,
            move |backend| backend.messages(&query),
            move |this, result, cx| {
                if generation != this.list_generation {
                    return;
                }
                this.list_loading = false;
                match result {
                    Ok(page) => {
                        this.messages.clear();
                        this.listed.clear();
                        this.fetched = 0;
                        this.merge_page(page.messages);
                        this.has_more = page.has_more;
                        this.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        let keep = this
                            .selected
                            .filter(|id| this.messages.iter().any(|m| m.id == *id));
                        match keep.or_else(|| this.messages.first().map(|m| m.id)) {
                            Some(id) => this.open_message(id, cx),
                            None => {
                                this.selected = None;
                                this.reader = ReaderState::Empty;
                            },
                        }
                    },
                    Err(error) => {
                        this.messages.clear();
                        this.has_more = false;
                        this.list_error = Some(error_chain(&error).into());
                    },
                }
            },
        );
        cx.notify();
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.list_loading || !self.has_more {
            return;
        }
        self.list_loading = true;
        let generation = self.list_generation;
        let query = self.current_query(self.fetched);
        self.run(
            cx,
            move |backend| backend.messages(&query),
            move |this, result, _| {
                if generation != this.list_generation {
                    return;
                }
                this.list_loading = false;
                match result {
                    Ok(page) => {
                        this.merge_page(page.messages);
                        this.has_more = page.has_more;
                    },
                    Err(error) => {
                        this.has_more = false;
                        this.list_error = Some(error_chain(&error).into());
                    },
                }
            },
        );
    }

    fn open_message(&mut self, id: MessageId, cx: &mut Context<Self>) {
        let Some(summary) = self.messages.iter().find(|m| m.id == id).cloned() else {
            return;
        };
        if self.selected == Some(id) && matches!(self.reader, ReaderState::Ready(_)) {
            return;
        }
        self.selected = Some(id);
        self.reader = ReaderState::Loading;
        self.reader_generation += 1;
        let generation = self.reader_generation;
        self.run(
            cx,
            move |backend| backend.thread(&summary),
            move |this, result, _| {
                if generation != this.reader_generation {
                    return;
                }
                match result {
                    Ok(thread) => {
                        // Expand the selected message and the latest one, like Gmail.
                        this.expanded.clear();
                        this.expanded.insert(id);
                        if let Some(last) = thread.messages.last() {
                            this.expanded.insert(last.summary.id);
                        }
                        let chat = thread.messages.iter().any(|m| !m.summary.is_email());
                        if chat {
                            this.expanded
                                .extend(thread.messages.iter().map(|m| m.summary.id));
                        }
                        this.reader_scroll.set_offset(point(px(0.), px(0.)));
                        this.reader = ReaderState::Ready(Arc::new(thread));
                    },
                    Err(error) => this.reader = ReaderState::Failed(error_chain(&error).into()),
                }
            },
        );
        cx.notify();
    }

    fn select_offset(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.messages.is_empty() {
            return;
        }
        let current = self
            .selected
            .and_then(|id| self.messages.iter().position(|m| m.id == id));
        let next = match current {
            Some(ix) => ix.saturating_add_signed(delta).min(self.messages.len() - 1),
            None => 0,
        };
        if let Some(id) = self.messages.get(next).map(|m| m.id) {
            self.list_scroll
                .scroll_to_item(next, ScrollStrategy::Nearest);
            self.open_message(id, cx);
        }
        if next + LOAD_AHEAD >= self.messages.len() {
            self.load_more(cx);
        }
    }

    fn set_mailbox(&mut self, mailbox: Mailbox, window: &mut Window, cx: &mut Context<Self>) {
        let searching = self.query.take().is_some();
        if searching {
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
        if self.mailbox == mailbox && !searching {
            return;
        }
        self.mailbox = mailbox;
        self.selected = None;
        self.reload_list(cx);
        window.focus(&self.focus, cx);
    }

    fn set_account(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        if self.account == account {
            return;
        }
        self.account = account;
        self.selected = None;
        self.refresh(cx);
    }

    fn on_search_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                self.search_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SEARCH_DEBOUNCE).await;
                    this.update(cx, |this, cx| this.apply_search(cx)).ok();
                }));
            },
            InputEvent::PressEnter { .. } => {
                self.search_task = None;
                self.apply_search(cx);
                window.focus(&self.focus, cx);
            },
            InputEvent::Focus | InputEvent::Blur => {},
        }
    }

    fn apply_search(&mut self, cx: &mut Context<Self>) {
        let text = self.search.read(cx).value().trim().to_owned();
        let query = (!text.is_empty()).then_some(text);
        if query == self.query {
            return;
        }
        self.query = query;
        self.selected = None;
        self.reload_list(cx);
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_task = None;
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        window.focus(&self.focus, cx);
        if self.query.take().is_some() {
            self.selected = None;
            self.reload_list(cx);
        }
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if cx.theme().is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        self.set_theme_mode(mode, window, cx);
    }

    pub(crate) fn set_theme_mode(
        &mut self,
        mode: ThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.follow_system = false;
        crate::theme::apply(Some(mode), Some(window), cx);
        cx.notify();
    }

    /// Keeps the newest message of each conversation, folding the state of
    /// older ones (unread, starred, attachments) into it.
    fn merge_page(&mut self, page: Vec<MessageSummary>) {
        self.fetched += page.len() as u64;
        for message in page {
            let Some(conversation) = message.conversation_id else {
                self.messages.push(message);
                continue;
            };
            if self.listed.insert(conversation) {
                self.messages.push(message);
            } else if let Some(row) = self
                .messages
                .iter_mut()
                .find(|row| row.conversation_id == Some(conversation))
            {
                row.unread |= message.unread;
                row.starred |= message.starred;
                row.has_attachments |= message.has_attachments;
            }
        }
    }

    fn toggle_expanded(&mut self, id: MessageId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        cx.notify();
    }

    fn expand_all(&mut self, cx: &mut Context<Self>) {
        if let ReaderState::Ready(thread) = &self.reader {
            let all: HashSet<MessageId> = thread.messages.iter().map(|m| m.summary.id).collect();
            if all.is_subset(&self.expanded) {
                self.expanded.clear();
                if let Some(last) = thread.messages.last() {
                    self.expanded.insert(last.summary.id);
                }
            } else {
                self.expanded = all;
            }
        }
        cx.notify();
    }

    /// Opens (or, with `save`, reveals a saved copy of) an attachment.
    fn open_attachment(
        &mut self,
        attachment: mv::Attachment,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(url) = attachment.remote_url() {
            cx.open_url(url);
            return;
        }
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let bytes = backend.attachment(&attachment)?;
            let name = format::safe_filename(&attachment.filename, "attachment");
            let dir = if save {
                downloads_dir()
            } else {
                std::env::temp_dir()
                    .join("msgvault-gpui")
                    .join(attachment.id.to_string())
            };
            std::fs::create_dir_all(&dir).map_err(|source| mv::Error::Io {
                path: dir.clone(),
                source,
            })?;
            let path = if save {
                unique_path(&dir, &name)
            } else {
                dir.join(&name)
            };
            std::fs::write(&path, bytes).map_err(|source| mv::Error::Io {
                path: path.clone(),
                source,
            })?;
            Ok::<_, mv::Error>(path)
        });
        cx.spawn_in(window, async move |_, cx| {
            let result = task.await;
            cx.update(|window, cx| match result {
                Ok(path) if save => {
                    cx.reveal_path(&path);
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                    window.push_notification(
                        Notification::success(format!(
                            "Saved {} to Downloads",
                            name.unwrap_or_default()
                        )),
                        cx,
                    );
                },
                Ok(path) => cx.open_with_system(&path),
                Err(error) => window.push_notification(
                    Notification::error(error_chain(&error)).title("Unable to open attachment"),
                    cx,
                ),
            })
            .ok();
        })
        .detach();
    }

    #[cfg(feature = "screenshot")]
    pub(crate) fn show_mailbox_named(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mailbox = match name.to_ascii_lowercase().as_str() {
            "inbox" => Mailbox::Inbox,
            "starred" => Mailbox::Starred,
            "sent" => Mailbox::Sent,
            "all" | "all mail" => Mailbox::AllMail,
            "attachments" => Mailbox::Attachments,
            kind @ ("whatsapp" | "slack" | "calendar_event") => Mailbox::Kind(kind.to_owned()),
            _ => Mailbox::Label(name.to_owned()),
        };
        self.set_mailbox(mailbox, window, cx);
    }

    #[cfg(feature = "screenshot")]
    pub(crate) fn search_for(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| {
            search.set_value(query.to_owned(), window, cx)
        });
        self.apply_search(cx);
    }

    /// Screenshots show a neutral archive path rather than the machine's
    /// temporary directory.
    #[cfg(feature = "screenshot")]
    pub(crate) fn set_display_location(&mut self, location: &str, cx: &mut Context<Self>) {
        if let Some(info) = &mut self.info {
            info.location = location.to_owned();
        }
        cx.notify();
    }

    #[cfg(feature = "screenshot")]
    pub(crate) fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.select_offset(delta, cx);
    }

    fn account_for(&self, id: Option<AccountId>) -> Option<&Account> {
        id.and_then(|id| self.accounts.iter().find(|a| a.id == id))
    }
}

fn downloads_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    home.map(|home| home.join("Downloads"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
        _ => (name, String::new()),
    };
    (1..1000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|path| !path.exists())
        .unwrap_or(candidate)
}

/// An error and its causes on one line, for display.
pub(crate) fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl Render for MailApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.connection {
            Connection::Ready => h_resizable("mail-layout")
                .child(
                    resizable_panel()
                        .size(px(250.))
                        .size_range(px(200.)..px(380.))
                        .child(self.render_sidebar(window, cx)),
                )
                .child(
                    resizable_panel()
                        .size(px(420.))
                        .size_range(px(320.)..px(720.))
                        .child(self.render_list(window, cx)),
                )
                .child(resizable_panel().child(self.render_reader(window, cx)))
                .into_any_element(),
            _ => self.render_welcome(window, cx).into_any_element(),
        };
        let notifications = Root::render_notification_layer(window, cx);
        let dialogs = Root::render_dialog_layer(window, cx);
        div()
            .id("mail-app")
            .key_context("MailApp")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Refresh, _, cx| {
                if this.backend.is_some() {
                    this.refresh(cx);
                } else {
                    this.connect(cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &ToggleTheme, window, cx| this.toggle_theme(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleRichText, _, cx| {
                this.rich_text = !this.rich_text;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |search, cx| {
                    search.focus(window, cx);
                    search.select_all(window, cx);
                });
            }))
            .on_action(
                cx.listener(|this, _: &ClearSearch, window, cx| this.clear_search(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_offset(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_offset(-1, cx)))
            .on_action(cx.listener(|this, _: &ExpandAll, _, cx| this.expand_all(cx)))
            .on_action(cx.listener(|this, _: &GoInbox, window, cx| {
                this.set_mailbox(Mailbox::Inbox, window, cx)
            }))
            .on_action(cx.listener(|this, _: &GoStarred, window, cx| {
                this.set_mailbox(Mailbox::Starred, window, cx)
            }))
            .on_action(cx.listener(|this, _: &GoSent, window, cx| {
                this.set_mailbox(Mailbox::Sent, window, cx)
            }))
            .on_action(cx.listener(|this, _: &GoAllMail, window, cx| {
                this.set_mailbox(Mailbox::AllMail, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_titlebar(window, cx))
            .child(div().flex_1().min_h_0().w_full().child(body))
            .children(dialogs)
            .children(notifications)
    }
}
