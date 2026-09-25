use {
    super::{LOAD_AHEAD, MailApp},
    crate::format,
    gpui_kit::{
        assets::IconName as Lucide,
        component::{
            ActiveTheme, Icon, Sizable as _, avatar::Avatar, h_flex,
            scroll::ScrollableElement as _, spinner::Spinner, v_flex,
        },
        prelude::FluentBuilder as _,
        *,
    },
    msgvault_core::{Mailbox, MessageSummary, display_kind, display_label},
    std::ops::Range,
};

pub(super) const ROW_HEIGHT: f32 = 92.;

impl MailApp {
    fn list_title(&self) -> String {
        match &self.query {
            Some(_) => "Search".into(),
            None => self.mailbox.title(),
        }
    }

    pub(super) fn render_list(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let subtitle = match (&self.query, self.list_loading && self.messages.is_empty()) {
            (_, true) => "Loading…".to_owned(),
            (Some(query), false) => {
                format!(
                    "{} for “{query}”",
                    format::plural(self.messages.len() as u64, "result", "results")
                ) + if self.has_more {
                    "+"
                } else {
                    ""
                }
            },
            (None, false) => {
                let loaded =
                    format::plural(self.messages.len() as u64, "conversation", "conversations");
                if self.has_more {
                    format!("{loaded}+")
                } else {
                    loaded
                }
            },
        };
        let account = self.account_for(self.account).map(|a| a.title().to_owned());

        let header = h_flex()
            .flex_none()
            .h(px(64.))
            .px_4()
            .gap_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .truncate()
                            .child(self.list_title()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(match account {
                                Some(account) => format!("{subtitle} · {account}"),
                                None => subtitle,
                            }),
                    ),
            )
            .when(self.list_loading, |header| {
                header.child(Spinner::new().small())
            });

        let body = if let Some(error) = &self.list_error {
            notice(
                Lucide::CircleAlert,
                "Unable to load messages",
                error.clone(),
                cx,
            )
            .into_any_element()
        } else if self.messages.is_empty() && !self.list_loading {
            let (title, detail) = match &self.query {
                Some(_) => (
                    "No results",
                    "Try different words, or operators like from: and has:attachment.",
                ),
                None if self.mailbox == Mailbox::Inbox => {
                    ("Inbox zero", "Nothing archived in this mailbox.")
                },
                None => ("Nothing here", "No archived messages in this view."),
            };
            notice(Lucide::Inbox, title, detail.into(), cx).into_any_element()
        } else {
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    uniform_list(
                        "messages",
                        self.messages.len(),
                        cx.processor(|this, range: Range<usize>, _, cx| {
                            this.render_rows(range, cx)
                        }),
                    )
                    .track_scroll(&self.list_scroll)
                    .size_full(),
                )
                .vertical_scrollbar(&self.list_scroll)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .child(header)
            .child(body)
    }

    fn render_rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if range.end + LOAD_AHEAD >= self.messages.len() && self.has_more && !self.list_loading {
            cx.spawn(async move |this, cx| {
                this.update(cx, |this, cx| this.load_more(cx)).ok();
            })
            .detach();
        }
        let now = chrono::Local::now();
        range
            .filter_map(|ix| {
                let message = self.messages.get(ix)?;
                Some(self.render_row(ix, message, now, cx))
            })
            .collect()
    }

    fn render_row(
        &self,
        ix: usize,
        message: &MessageSummary,
        now: chrono::DateTime<chrono::Local>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let selected = self.selected == Some(message.id);
        let id = message.id;
        let sender = message.from.label().to_owned();
        // Chats lead with the conversation, like a messaging app.
        let (title, sender) = if message.is_email() {
            (sender, None)
        } else {
            (message.subject.clone(), Some(sender))
        };
        let date = message
            .sent_at
            .map(|t| format::list_date(t, now))
            .unwrap_or_default();
        let subject = match sender {
            Some(sender) => sender,
            None if message.subject.trim().is_empty() => "(no subject)".to_owned(),
            None => message.subject.clone(),
        };
        let title = if title.trim().is_empty() {
            display_kind(&message.message_type)
        } else {
            title
        };
        let weight = if message.unread {
            FontWeight::BOLD
        } else {
            FontWeight::MEDIUM
        };
        let muted = theme.muted_foreground;

        let mut badges = h_flex().gap_1().flex_none().items_center();
        if let Some(count) = message
            .thread_count
            .filter(|n| *n > 1 && message.is_email())
        {
            badges = badges.child(
                div()
                    .text_xs()
                    .px_1()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(theme.border)
                    .text_color(muted)
                    .child(count.to_string()),
            );
        }
        if message.has_attachments {
            badges = badges.child(Icon::new(Lucide::Paperclip).size(px(13.)).text_color(muted));
        }
        if message.starred {
            badges = badges.child(
                Icon::new(Lucide::Star)
                    .size(px(13.))
                    .text_color(theme.warning),
            );
        }
        if message.removed_upstream {
            badges = badges.child(Icon::new(Lucide::CloudOff).size(px(13.)).text_color(muted));
        }

        let chips = message.visible_labels().take(2).map(|name| {
            let color = format::label_color(name, None);
            div()
                .flex_none()
                .text_xs()
                .px_1p5()
                .rounded(px(4.))
                .bg(color.opacity(0.14))
                .text_color(color)
                .child(display_label(name))
        });

        div()
            .id(("message", ix))
            .h(px(ROW_HEIGHT))
            .w_full()
            .px_3()
            .py_2p5()
            .flex()
            .gap_3()
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border.opacity(0.6))
            .relative()
            .map(|row| {
                if selected {
                    row.bg(theme.accent).child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(3.))
                            .bg(theme.primary),
                    )
                } else {
                    row.hover(|row| row.bg(theme.accent.opacity(0.5)))
                }
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.open_message(id, cx);
            }))
            .child(
                h_flex()
                    .flex_none()
                    .items_start()
                    .gap_1p5()
                    .child(
                        div()
                            .mt(px(14.))
                            .size(px(7.))
                            .rounded_full()
                            .when(message.unread, |dot| dot.bg(theme.primary)),
                    )
                    .child(Avatar::new().name(title.clone()).with_size(px(34.))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(weight)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(if message.unread {
                                        theme.primary
                                    } else {
                                        muted
                                    })
                                    .child(date),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(if message.unread {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(subject),
                            )
                            .child(badges),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .text_color(muted)
                                    .child(message.snippet.clone()),
                            )
                            .children(chips),
                    ),
            )
            .into_any_element()
    }
}

/// A centered icon, title and explanation for empty and error states.
pub(super) fn notice(
    icon: Lucide,
    title: &'static str,
    detail: SharedString,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .flex_1()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p_8()
        .child(
            div()
                .size(px(56.))
                .rounded_full()
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .size(px(24.))
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(div().mt_2().font_weight(FontWeight::SEMIBOLD).child(title))
        .child(
            div()
                .max_w(px(320.))
                .text_sm()
                .text_center()
                .text_color(theme.muted_foreground)
                .child(detail),
        )
}
