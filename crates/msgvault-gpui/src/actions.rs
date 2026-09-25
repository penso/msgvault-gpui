use gpui_kit::{App, KeyBinding, Menu, MenuItem};

gpui_kit::actions!(msgvault, [
    Quit,
    Refresh,
    ToggleTheme,
    ToggleRichText,
    FocusSearch,
    ClearSearch,
    SelectNext,
    SelectPrevious,
    GoInbox,
    GoStarred,
    GoSent,
    GoAllMail,
    ExpandAll,
]);

/// Single-key shortcuts must not fire while typing in the search field.
const LIST: &str = "MailApp && !Input";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-r", Refresh, Some("MailApp")),
        KeyBinding::new("cmd-shift-l", ToggleTheme, Some("MailApp")),
        KeyBinding::new("cmd-shift-h", ToggleRichText, Some("MailApp")),
        KeyBinding::new("cmd-f", FocusSearch, Some("MailApp")),
        KeyBinding::new("cmd-k", FocusSearch, Some("MailApp")),
        KeyBinding::new("/", FocusSearch, Some(LIST)),
        KeyBinding::new("escape", ClearSearch, Some("MailApp && Input")),
        KeyBinding::new("escape", ClearSearch, Some(LIST)),
        KeyBinding::new("j", SelectNext, Some(LIST)),
        KeyBinding::new("down", SelectNext, Some(LIST)),
        KeyBinding::new("k", SelectPrevious, Some(LIST)),
        KeyBinding::new("up", SelectPrevious, Some(LIST)),
        KeyBinding::new(";", ExpandAll, Some(LIST)),
        KeyBinding::new("cmd-1", GoInbox, Some("MailApp")),
        KeyBinding::new("cmd-2", GoStarred, Some("MailApp")),
        KeyBinding::new("cmd-3", GoSent, Some("MailApp")),
        KeyBinding::new("cmd-4", GoAllMail, Some("MailApp")),
    ]);
}

pub(crate) fn menus() -> Vec<Menu> {
    vec![
        Menu::new("msgvault").items([MenuItem::action("Quit msgvault", Quit)]),
        Menu::new("View").items([
            MenuItem::action("Refresh", Refresh),
            MenuItem::action("Toggle Dark Mode", ToggleTheme),
            MenuItem::action("Toggle Rich Text", ToggleRichText),
            MenuItem::separator(),
            MenuItem::action("Expand Conversation", ExpandAll),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Inbox", GoInbox),
            MenuItem::action("Starred", GoStarred),
            MenuItem::action("Sent", GoSent),
            MenuItem::action("All Mail", GoAllMail),
            MenuItem::separator(),
            MenuItem::action("Next Message", SelectNext),
            MenuItem::action("Previous Message", SelectPrevious),
            MenuItem::action("Search", FocusSearch),
        ]),
    ]
}
