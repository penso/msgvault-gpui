//! Icons: gpui-component's default set plus the Lucide icons a mail client needs.

use {
    gpui_kit::{AssetSource, SharedString},
    std::borrow::Cow,
};

gpui_kit::assets::icon_assets!(MailIcons, [
    Archive,
    ArrowUpRight,
    AtSign,
    CalendarDays,
    CircleAlert,
    Clock,
    CloudOff,
    Code,
    Database,
    Download,
    Eye,
    FileArchive,
    FileCode,
    FileDown,
    FileText,
    Flag,
    FolderX,
    Hash,
    Image,
    Inbox,
    Mail,
    MailOpen,
    Mails,
    MessageCircle,
    MessageSquareText,
    MessagesSquare,
    OctagonAlert,
    Paperclip,
    RefreshCw,
    Reply,
    Send,
    Server,
    Sparkles,
    Star,
    SunMoon,
    Tag,
    Tags,
    Trash,
    Users,
    X,
]);

pub(crate) struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = MailIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(MailIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
