use {
    super::MailApp,
    crate::format,
    gpui_kit::{
        assets::IconName as Lucide,
        component::{
            ActiveTheme, Icon, IconName, h_flex,
            sidebar::{
                Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
            },
            v_flex,
        },
        *,
    },
    msgvault_core::{Account, AccountId, LabelKind, Mailbox, display_kind},
};

/// A count pill for sidebar rows; hidden at zero.
fn count_suffix(
    count: u64,
    strong: bool,
) -> impl Fn(&mut Window, &mut App) -> AnyElement + 'static {
    move |_, cx| {
        if count == 0 {
            return div().into_any_element();
        }
        let theme = cx.theme();
        let pill = div()
            .text_xs()
            .px_1p5()
            .min_w(px(20.))
            .flex()
            .justify_center()
            .rounded_full()
            .child(format::count(count));
        if strong {
            pill.bg(theme.primary)
                .text_color(theme.primary_foreground)
                .font_weight(FontWeight::SEMIBOLD)
                .into_any_element()
        } else {
            pill.text_color(theme.muted_foreground).into_any_element()
        }
    }
}

fn account_icon(kind: &str) -> Icon {
    let icon = match kind {
        "gmail" | "imap" | "fastmail" | "microsoft" | "outlook" | "mbox" | "emlx" | "pst" => {
            Lucide::Mail
        },
        "whatsapp" | "imessage" | "apple_messages" | "google_messages" | "sms" | "beeper"
        | "google_voice" => Lucide::MessageCircle,
        "slack" | "discord" | "teams" | "google_chat" | "matrix" => Lucide::Hash,
        "gcal" | "calendar" => Lucide::CalendarDays,
        _ => Lucide::Database,
    };
    Icon::new(icon)
}

fn kind_icon(kind: &str) -> Icon {
    match kind {
        "calendar_event" => Icon::new(Lucide::CalendarDays),
        "meeting_transcript" => Icon::new(Lucide::MessageSquareText),
        "slack" | "discord" | "teams" | "google_chat" | "matrix" => Icon::new(Lucide::Hash),
        _ => Icon::new(Lucide::MessagesSquare),
    }
}

impl MailApp {
    fn mailbox_item(
        &self,
        mailbox: Mailbox,
        icon: impl Into<Icon>,
        count: u64,
        strong: bool,
        cx: &mut Context<Self>,
    ) -> SidebarMenuItem {
        let active = self.query.is_none() && self.mailbox == mailbox;
        let this = cx.entity().downgrade();
        SidebarMenuItem::new(mailbox.title())
            .icon(icon)
            .active(active)
            .suffix(count_suffix(count, strong))
            .on_click(move |_, window, cx| {
                let mailbox = mailbox.clone();
                this.update(cx, |this, cx| this.set_mailbox(mailbox, window, cx))
                    .ok();
            })
    }

    fn account_item(&self, account: Option<&Account>, cx: &mut Context<Self>) -> SidebarMenuItem {
        let id: Option<AccountId> = account.map(|a| a.id);
        let (title, icon, count) = match account {
            Some(account) => (
                account.title().to_owned(),
                account_icon(&account.kind),
                account.message_count,
            ),
            None => (
                "All Accounts".to_owned(),
                Icon::new(Lucide::Users),
                self.stats.total_messages,
            ),
        };
        let this = cx.entity().downgrade();
        SidebarMenuItem::new(title)
            .icon(icon)
            .active(self.account == id)
            .suffix(count_suffix(count, false))
            .on_click(move |_, _, cx| {
                this.update(cx, |this, cx| this.set_account(id, cx)).ok();
            })
    }

    pub(super) fn render_sidebar(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let counts = self.counts.clone();
        let mailboxes = SidebarMenu::new().children([
            self.mailbox_item(
                Mailbox::Inbox,
                IconName::Inbox,
                counts.inbox_unread,
                true,
                cx,
            ),
            self.mailbox_item(Mailbox::Starred, Lucide::Star, counts.starred, false, cx),
            self.mailbox_item(Mailbox::Unread, Lucide::MailOpen, 0, false, cx),
            self.mailbox_item(Mailbox::Important, Lucide::Flag, 0, false, cx),
            self.mailbox_item(Mailbox::Sent, Lucide::Send, 0, false, cx),
            self.mailbox_item(Mailbox::Drafts, IconName::File, counts.drafts, false, cx),
            self.mailbox_item(Mailbox::AllMail, Lucide::Mails, counts.all_mail, false, cx),
            self.mailbox_item(
                Mailbox::Attachments,
                Lucide::Paperclip,
                counts.attachments,
                false,
                cx,
            ),
            self.mailbox_item(Mailbox::Spam, Lucide::OctagonAlert, 0, false, cx),
            self.mailbox_item(Mailbox::Trash, Lucide::Trash, 0, false, cx),
        ]);

        let mut sidebar = Sidebar::new("sidebar")
            .w(relative(1.))
            .border_0()
            .collapsible(false)
            .header(self.render_sidebar_header(cx))
            .child(SidebarGroup::new("Mailboxes").child(mailboxes));

        let labels: Vec<_> = self
            .labels
            .iter()
            .filter(|l| l.kind == LabelKind::User && l.count > 0)
            .cloned()
            .collect();
        if !labels.is_empty() {
            let items: Vec<SidebarMenuItem> = labels
                .iter()
                .map(|label| {
                    let color = format::label_color(&label.name, label.color.as_deref());
                    let dot = Icon::new(Lucide::Tag).text_color(color);
                    self.mailbox_item(
                        Mailbox::Label(label.name.clone()),
                        dot,
                        label.count,
                        false,
                        cx,
                    )
                })
                .collect();
            sidebar = sidebar
                .child(SidebarGroup::new("Labels").child(SidebarMenu::new().children(items)));
        }

        let categories: Vec<SidebarMenuItem> = self
            .labels
            .iter()
            .filter(|l| l.kind == LabelKind::Category && l.count > 0)
            .map(|label| {
                self.mailbox_item(
                    Mailbox::Label(label.name.clone()),
                    Lucide::Tags,
                    label.count,
                    false,
                    cx,
                )
            })
            .collect();
        if !categories.is_empty() {
            sidebar = sidebar.child(
                SidebarGroup::new("Categories").child(SidebarMenu::new().children(categories)),
            );
        }

        let mut other: Vec<SidebarMenuItem> = counts
            .kinds
            .iter()
            .map(|(kind, count)| {
                self.mailbox_item(
                    Mailbox::Kind(kind.clone()),
                    kind_icon(kind),
                    *count,
                    false,
                    cx,
                )
            })
            .collect();
        if counts.removed_upstream > 0 {
            other.push(self.mailbox_item(
                Mailbox::RemovedUpstream,
                Lucide::CloudOff,
                counts.removed_upstream,
                false,
                cx,
            ));
        }
        if !other.is_empty() {
            sidebar = sidebar
                .child(SidebarGroup::new("Archive").child(SidebarMenu::new().children(other)));
        }

        let mut accounts = vec![self.account_item(None, cx)];
        let all_accounts = self.accounts.clone();
        accounts.extend(all_accounts.iter().map(|a| self.account_item(Some(a), cx)));
        sidebar = sidebar
            .child(SidebarGroup::new("Accounts").child(SidebarMenu::new().children(accounts)));

        let theme = cx.theme();
        let footer = SidebarFooter::new().child(
            v_flex()
                .w_full()
                .gap_0p5()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!(
                    "{} · {}",
                    format::plural(self.stats.total_messages, "message", "messages"),
                    format::plural(self.stats.total_threads, "thread", "threads"),
                ))
                .child(format!(
                    "{} · {} on disk",
                    format::plural(self.stats.total_attachments, "attachment", "attachments"),
                    format::bytes(self.stats.database_size_bytes)
                )),
        );
        div()
            .size_full()
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(sidebar.footer(footer))
    }

    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let account = self.account_for(self.account);
        let (title, subtitle) = match account {
            Some(account) => (
                account.title().to_owned(),
                format!("{} · {}", display_kind(&account.kind), account.identifier),
            ),
            None => (
                "All Accounts".to_owned(),
                format::plural(self.accounts.len() as u64, "source", "sources"),
            ),
        };
        let icon = account.map_or_else(|| Icon::new(Lucide::Archive), |a| account_icon(&a.kind));
        SidebarHeader::new().child(
            h_flex()
                .w_full()
                .gap_2p5()
                .items_center()
                .child(
                    div()
                        .size_8()
                        .flex_none()
                        .rounded(px(8.))
                        .bg(theme.sidebar_primary)
                        .text_color(theme.sidebar_primary_foreground)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon.size(px(16.))),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(subtitle),
                        ),
                ),
        )
    }
}
