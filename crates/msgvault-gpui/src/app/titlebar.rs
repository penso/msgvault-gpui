use {
    super::MailApp,
    crate::actions::ToggleTheme,
    gpui_kit::{
        assets::IconName as Lucide,
        component::{
            ActiveTheme, Icon, IconName, Sizable as _, TitleBar,
            button::{Button, ButtonVariants as _},
            h_flex,
            input::Input,
        },
        prelude::FluentBuilder as _,
        *,
    },
    msgvault_core::BackendKind,
};

impl MailApp {
    pub(super) fn render_titlebar(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let brand = h_flex()
            .gap_2()
            .items_center()
            .flex_none()
            .child(
                div()
                    .size(px(22.))
                    .rounded(px(6.))
                    .bg(theme.primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(Lucide::Mails)
                            .size(px(13.))
                            .text_color(theme.primary_foreground),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("msgvault"),
            );

        let source = self.info.as_ref().map(|info| {
            let (icon, label) = match info.kind {
                BackendKind::Sqlite => (Lucide::Database, "SQLite"),
                BackendKind::Daemon => (Lucide::Server, "Daemon"),
            };
            h_flex()
                .id("backend-badge")
                .gap_1p5()
                .items_center()
                .px_2()
                .py_0p5()
                .rounded_full()
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(Icon::new(icon).size(px(12.)))
                .child(div().font_weight(FontWeight::MEDIUM).child(label))
                .child(
                    div()
                        .max_w(px(220.))
                        .truncate()
                        .child(info.location.clone()),
                )
        });

        let dark = theme.is_dark();
        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr_3()
                .gap_4()
                .items_center()
                .child(brand)
                .child(
                    div().flex_1().flex().justify_center().child(
                        Input::new(&self.search)
                            .id("search")
                            .prefix(
                                Icon::new(IconName::Search)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            )
                            .cleanable(true)
                            .small()
                            .w(px(460.)),
                    ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_none()
                        .children(source)
                        .when(self.backend.is_some(), |row| {
                            row.child(
                                Button::new("refresh")
                                    .ghost()
                                    .small()
                                    .icon(Lucide::RefreshCw)
                                    .tooltip("Refresh (⌘R)")
                                    .loading(self.list_loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                            )
                        })
                        .child(
                            Button::new("theme")
                                .ghost()
                                .small()
                                .icon(if dark {
                                    IconName::Sun
                                } else {
                                    IconName::Moon
                                })
                                .tooltip("Toggle dark mode (⌘⇧L)")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(ToggleTheme), cx)
                                }),
                        ),
                ),
        )
    }
}
