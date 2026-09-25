use {
    super::{MailApp, ReaderState, list::notice},
    crate::{
        actions::{ExpandAll, ToggleRichText},
        format,
    },
    gpui_kit::{
        assets::IconName as Lucide,
        base::SelectableText,
        component::{
            ActiveTheme, Icon, Sizable as _,
            avatar::Avatar,
            button::{Button, ButtonVariants as _},
            h_flex,
            scroll::ScrollableElement as _,
            spinner::Spinner,
            text::TextView,
            v_flex,
        },
        prelude::FluentBuilder as _,
        *,
    },
    msgvault_core::{Address, Attachment, MessageDetail, Thread, display_kind, display_label},
};

/// HTML larger than this renders as plain text; the rich view is for mail, not archives of the web.
const HTML_LIMIT: usize = 400_000;
const READER_WIDTH: f32 = 880.;

fn attachment_icon(mime: &str) -> Lucide {
    match mime {
        m if m.starts_with("image/") => Lucide::Image,
        "text/calendar" => Lucide::CalendarDays,
        m if m.contains("zip") || m.contains("compressed") || m.contains("tar") => {
            Lucide::FileArchive
        },
        m if m.contains("pdf") || m.starts_with("text/") || m.contains("document") => {
            Lucide::FileText
        },
        _ => Lucide::FileDown,
    }
}

fn join_addresses(list: &[Address]) -> String {
    list.iter()
        .map(Address::label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A readable plain rendering of an HTML body, for the plain-text view.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut tag = String::new();
    let mut in_tag = false;
    let mut skip = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            },
            '>' if in_tag => {
                in_tag = false;
                let name = tag
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                match name.as_str() {
                    "style" | "script" | "head" => skip = !tag.starts_with('/'),
                    "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "blockquote"
                    | "table" => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                        if name == "li" && !tag.starts_with('/') {
                            out.push_str("• ");
                        }
                    },
                    "td" | "th" => out.push('\t'),
                    _ => {},
                }
            },
            c if in_tag => tag.push(c),
            c if !skip => out.push(c),
            _ => {},
        }
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    let mut lines: Vec<&str> = Vec::new();
    for line in decoded.lines().map(str::trim_end) {
        if line.trim().is_empty() && lines.last().is_some_and(|l| l.trim().is_empty()) {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_owned()
}

impl MailApp {
    pub(super) fn render_reader(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content = match &self.reader {
            ReaderState::Empty => notice(
                Lucide::Mail,
                "No message selected",
                "Pick a conversation from the list. Use J and K to move, / to search.".into(),
                cx,
            )
            .into_any_element(),
            ReaderState::Loading => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(Spinner::new())
                .into_any_element(),
            ReaderState::Failed(error) => notice(
                Lucide::CircleAlert,
                "Unable to open message",
                error.clone(),
                cx,
            )
            .into_any_element(),
            ReaderState::Ready(thread) => {
                let thread = thread.clone();
                self.render_thread(&thread, cx).into_any_element()
            },
        };
        div().size_full().bg(cx.theme().background).child(content)
    }

    fn render_thread(&self, thread: &Thread, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let chat = thread.messages.iter().any(|m| !m.summary.is_email());
        let kind = thread
            .messages
            .first()
            .map(|m| display_kind(&m.summary.message_type))
            .unwrap_or_default();
        let subject = if thread.subject.trim().is_empty() {
            "(no subject)".to_owned()
        } else {
            thread.subject.clone()
        };
        let mut labels: Vec<&str> = Vec::new();
        for message in &thread.messages {
            for label in message.summary.visible_labels() {
                if !labels.contains(&label) {
                    labels.push(label);
                }
            }
        }
        let account = thread
            .messages
            .first()
            .and_then(|m| self.account_for(m.summary.account_id))
            .map(|a| a.title().to_owned());
        let mut meta = vec![format::plural(thread.total, "message", "messages")];
        if chat {
            meta.insert(0, kind);
        }
        meta.extend(account.filter(|a| !meta.contains(a)));
        let meta = meta.join(" · ");

        let has_html = thread.messages.iter().any(|m| m.body_html.is_some());
        let toolbar = h_flex()
            .gap_1()
            .flex_none()
            .when(has_html, |bar| {
                bar.child(
                    Button::new("rich-text")
                        .ghost()
                        .small()
                        .icon(if self.rich_text {
                            Lucide::FileCode
                        } else {
                            Lucide::FileText
                        })
                        .tooltip(if self.rich_text {
                            "Show plain text (⌘⇧H)"
                        } else {
                            "Show formatted (⌘⇧H)"
                        })
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(ToggleRichText), cx)
                        }),
                )
            })
            .when(!chat && thread.messages.len() > 1, |bar| {
                bar.child(
                    Button::new("expand-all")
                        .ghost()
                        .small()
                        .icon(Lucide::Mails)
                        .tooltip("Expand or collapse all (;)")
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(ExpandAll), cx)),
                )
            });

        let header = v_flex()
            .gap_2()
            .pb_2()
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .line_height(relative(1.25))
                            .child(subject),
                    )
                    .child(toolbar),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .children(labels.into_iter().map(|name| {
                        let color = format::label_color(name, None);
                        h_flex()
                            .gap_1()
                            .items_center()
                            .text_xs()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(color.opacity(0.14))
                            .text_color(color)
                            .child(div().size(px(6.)).rounded_full().bg(color))
                            .child(display_label(name))
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(meta),
                    ),
            );

        let body = if chat {
            self.render_chat(thread, cx).into_any_element()
        } else {
            v_flex()
                .gap_3()
                .children(
                    thread
                        .messages
                        .iter()
                        .map(|message| self.render_email(message, thread.messages.len() > 1, cx)),
                )
                .into_any_element()
        };

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("reader-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.reader_scroll)
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(READER_WIDTH))
                            .mx_auto()
                            .px_8()
                            .pt_6()
                            .pb_12()
                            .gap_4()
                            .child(header)
                            .child(body),
                    ),
            )
            .vertical_scrollbar(&self.reader_scroll)
    }

    fn render_email(
        &self,
        message: &MessageDetail,
        collapsible: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let summary = &message.summary;
        let id = summary.id;
        let expanded = !collapsible || self.expanded.contains(&id);
        let sender = summary.from.label().to_owned();
        let date = summary.sent_at.map(format::full_date).unwrap_or_default();
        let toggle = cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_expanded(id, cx));

        if !expanded {
            return h_flex()
                .id(("collapsed", id.0 as usize))
                .gap_3()
                .px_4()
                .py_3()
                .items_center()
                .cursor_pointer()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .hover(|row| row.bg(theme.accent.opacity(0.5)))
                .on_click(toggle)
                .child(Avatar::new().name(sender.clone()).with_size(px(28.)))
                .child(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(sender),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(summary.snippet.clone()),
                )
                .when(summary.has_attachments, |row| {
                    row.child(
                        Icon::new(Lucide::Paperclip)
                            .size(px(13.))
                            .text_color(theme.muted_foreground),
                    )
                })
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(date),
                )
                .into_any_element();
        }

        let mut recipients = v_flex()
            .gap_0p5()
            .text_xs()
            .text_color(theme.muted_foreground);
        for (label, list) in [
            ("to", &message.to),
            ("cc", &message.cc),
            ("bcc", &message.bcc),
        ] {
            if !list.is_empty() {
                recipients = recipients.child(
                    h_flex()
                        .gap_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex_none()
                                .font_weight(FontWeight::MEDIUM)
                                .child(label),
                        )
                        .child(div().min_w_0().truncate().child(join_addresses(list))),
                );
            }
        }
        let address = (!summary.from.address.is_empty() && summary.from.address != sender)
            .then(|| summary.from.address.clone());

        let header = h_flex()
            .id(("header", id.0 as usize))
            .gap_3()
            .p_4()
            .items_start()
            .when(collapsible, |row| row.cursor_pointer().on_click(toggle))
            .child(Avatar::new().name(sender.clone()).with_size(px(40.)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_baseline()
                            .min_w_0()
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(sender),
                            )
                            .children(address.map(|a| {
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(a)
                            })),
                    )
                    .child(recipients),
            )
            .child(
                v_flex()
                    .flex_none()
                    .items_end()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(date),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .when(summary.starred, |row| {
                                row.child(
                                    Icon::new(Lucide::Star)
                                        .size(px(13.))
                                        .text_color(theme.warning),
                                )
                            })
                            .when(summary.removed_upstream, |row| {
                                row.child(
                                    h_flex()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(Icon::new(Lucide::CloudOff).size(px(12.)))
                                        .child("Deleted upstream"),
                                )
                            }),
                    ),
            );

        let files: Vec<&Attachment> = message.attachments.iter().filter(|a| !a.inline).collect();
        v_flex()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_xs()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .px_4()
                    .pb_5()
                    .pl(px(68.))
                    .child(self.render_body(message, cx)),
            )
            .when(!files.is_empty(), |card| {
                card.child(
                    div()
                        .px_4()
                        .pb_4()
                        .pl(px(68.))
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(
                            files
                                .into_iter()
                                .enumerate()
                                .map(|(ix, a)| self.render_attachment(id.0, ix, a, cx)),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_body(&self, message: &MessageDetail, cx: &Context<Self>) -> AnyElement {
        let id = message.summary.id.0 as usize;
        let html = message
            .body_html
            .as_deref()
            .filter(|h| h.len() <= HTML_LIMIT);
        if self.rich_text
            && let Some(html) = html
        {
            return TextView::html(("html", id), html.to_owned())
                .selectable(true)
                .w_full()
                .into_any_element();
        }
        let text = match (&message.body_text, &message.body_html) {
            (Some(text), _) => text.clone(),
            (None, Some(html)) => html_to_text(html),
            (None, None) if !message.summary.snippet.is_empty() => message.summary.snippet.clone(),
            (None, None) => {
                return div()
                    .text_sm()
                    .italic()
                    .text_color(cx.theme().muted_foreground)
                    .child("This message has no archived body.")
                    .into_any_element();
            },
        };
        div()
            .w_full()
            .text_sm()
            .line_height(relative(1.6))
            .child(SelectableText::new(
                ("text", id),
                text.trim_end().to_owned(),
            ))
            .into_any_element()
    }

    fn render_attachment(
        &self,
        message: i64,
        ix: usize,
        attachment: &Attachment,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let key = (message as usize) * 1000 + ix;
        let open = attachment.clone();
        let save = attachment.clone();
        let name = if attachment.filename.is_empty() {
            "attachment".to_owned()
        } else {
            attachment.filename.clone()
        };
        let remote = attachment.remote_url().is_some();
        h_flex()
            .id(("attachment", key))
            .gap_2p5()
            .pl_3()
            .pr_1()
            .py_1p5()
            .max_w(px(300.))
            .items_center()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted.opacity(0.5))
            .cursor_pointer()
            .hover(|chip| chip.bg(theme.accent))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.open_attachment(open.clone(), false, window, cx);
            }))
            .child(
                div()
                    .size(px(30.))
                    .flex_none()
                    .rounded(px(6.))
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(attachment_icon(&attachment.mime_type))
                            .size(px(16.))
                            .text_color(theme.primary),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if remote {
                                "Remote file".to_owned()
                            } else {
                                format::bytes(attachment.size_bytes)
                            }),
                    ),
            )
            .when(!remote, |chip| {
                chip.child(
                    Button::new(("save", key))
                        .ghost()
                        .xsmall()
                        .icon(Lucide::Download)
                        .tooltip("Save to Downloads")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_attachment(save.clone(), true, window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn render_chat(&self, thread: &Thread, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut column = v_flex().gap_1().pt_2();
        let mut previous: Option<(&str, bool)> = None;
        for message in &thread.messages {
            let summary = &message.summary;
            let mine = message.is_from_me;
            let sender = summary.from.label();
            let first_of_group = previous != Some((sender, mine));
            previous = Some((sender, mine));
            let text = message
                .body_text
                .clone()
                .or_else(|| message.body_html.as_deref().map(html_to_text))
                .unwrap_or_else(|| summary.snippet.clone());
            let time = summary
                .sent_at
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%-I:%M %p")
                        .to_string()
                })
                .unwrap_or_default();
            let bubble = div()
                .max_w(relative(0.72))
                .px_3()
                .py_2()
                .rounded(px(16.))
                .text_sm()
                .line_height(relative(1.45))
                .map(|b| {
                    if mine {
                        b.bg(theme.primary).text_color(theme.primary_foreground)
                    } else {
                        b.bg(theme.muted).text_color(theme.foreground)
                    }
                })
                .child(SelectableText::new(("chat", summary.id.0 as usize), text));
            let row = h_flex()
                .w_full()
                .gap_2()
                .items_end()
                .when(mine, |row| row.justify_end())
                .when(!mine, |row| {
                    row.child(
                        div()
                            .size(px(28.))
                            .flex_none()
                            .when(first_of_group, |slot| {
                                slot.child(Avatar::new().name(sender.to_owned()).with_size(px(28.)))
                            }),
                    )
                })
                .child(bubble)
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(time),
                );
            if first_of_group {
                column = column.child(
                    div()
                        .mt_3()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .when(mine, |label| label.text_right().pr(px(60.)))
                        .when(!mine, |label| label.pl(px(36.)))
                        .child(if mine {
                            "You".to_owned()
                        } else {
                            sender.to_owned()
                        }),
                );
            }
            column = column.child(row);
        }
        column
    }
}

#[cfg(test)]
mod tests {
    use super::html_to_text;

    #[test]
    fn html_becomes_readable_text() {
        let text = html_to_text(
            "<html><head><style>p{color:red}</style></head><body><h1>Hi</h1><p>One &amp; two</p><ul><li>a</li><li>b</li></ul></body></html>",
        );
        assert_eq!(text, "Hi\nOne & two\n• a\n• b");
    }
}
