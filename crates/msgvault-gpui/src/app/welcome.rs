use {
    super::{Connection, MailApp},
    gpui_kit::{
        assets::IconName as Lucide,
        component::{
            ActiveTheme, Icon,
            button::{Button, ButtonVariants as _},
            h_flex,
            spinner::Spinner,
            v_flex,
        },
        *,
    },
    msgvault_core::display_path,
};

impl MailApp {
    pub(super) fn render_welcome(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let (title, detail): (&str, String) = match &self.connection {
            Connection::Connecting => {
                return v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Spinner::new())
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("Opening {}…", display_path(&self.launch.home))),
                    )
                    .into_any_element();
            },
            Connection::Missing { path } => (
                "No msgvault archive yet",
                format!(
                    "Nothing was found at {}. Archive your mail with `msgvault sync`, point this app at another archive with --home, or explore a generated demo.",
                    display_path(path)
                ),
            ),
            Connection::Failed(error) => ("Unable to open the archive", error.to_string()),
            Connection::Ready => ("", String::new()),
        };

        let code = |text: &'static str| {
            div()
                .px_2()
                .py_1()
                .rounded(px(6.))
                .bg(theme.muted)
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .child(text)
        };

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(520.))
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .size(px(72.))
                            .rounded(px(20.))
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .shadow_lg()
                            .child(
                                Icon::new(Lucide::Mails)
                                    .size(px(34.))
                                    .text_color(theme.primary_foreground),
                            ),
                    )
                    .child(
                        div()
                            .mt_2()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .line_height(relative(1.5))
                            .text_color(theme.muted_foreground)
                            .child(detail),
                    )
                    .child(
                        h_flex()
                            .mt_2()
                            .gap_2()
                            .child(
                                Button::new("demo")
                                    .primary()
                                    .icon(Lucide::Sparkles)
                                    .label("Open demo archive")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_demo(cx))),
                            )
                            .child(
                                Button::new("retry")
                                    .outline()
                                    .icon(Lucide::RefreshCw)
                                    .label("Try again")
                                    .on_click(cx.listener(|this, _, _, cx| this.connect(cx))),
                            ),
                    )
                    .child(
                        v_flex()
                            .mt_6()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Get started with msgvault")
                            .child(code("msgvault add-account you@gmail.com"))
                            .child(code("msgvault sync")),
                    ),
            )
            .into_any_element()
    }
}
