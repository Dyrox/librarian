//! UI language. Strings are written in English at the call site and looked
//! up here (`t("Open")`); anything without a translation stays English.
//! Strings with numbers in them have their own small functions below.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    System,
    En,
    Zh,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::System => "system",
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "en" => Lang::En,
            "zh" => Lang::Zh,
            _ => Lang::System,
        }
    }
}

static ZH: AtomicBool = AtomicBool::new(false);

pub fn set(lang: Lang) {
    let zh = match lang {
        Lang::System => sys_locale::get_locales().next().is_some_and(|l| l.to_lowercase().starts_with("zh")),
        Lang::En => false,
        Lang::Zh => true,
    };
    ZH.store(zh, Ordering::Relaxed);
}

pub fn zh() -> bool {
    ZH.load(Ordering::Relaxed)
}

pub fn t(s: &'static str) -> &'static str {
    if !zh() {
        return s;
    }
    match s {
        // Sidebar & header
        "All Projects" => "全部工程",
        "Last 30 Days" => "最近 30 天",
        "With Demos" => "有 Demo",
        "New Bounces" => "新导出",
        "Issues" => "问题",
        "DAWS" => "DAW",
        "TAGS" => "标签",
        "FOLDERS" => "文件夹",
        "No folder" => "未选择文件夹",
        "Change folder…" => "更换文件夹…",
        "Settings" => "设置",
        "Search projects" => "搜索工程",
        "Recent" => "最近",
        "Name" => "名称",
        "Scanning…" => "正在扫描…",
        "Everything follows your conventions" => "一切都符合你的整理习惯",

        // Welcome
        "Choose your music projects folder" => "选择你的音乐工程文件夹",
        "Your projects, at a glance" => "所有工程，一目了然",
        "Choose the folder where you keep your music projects.\nLibrarian finds every Ableton Live and FL Studio project inside it, draws its arrangement and lines up your latest bounce for a quick listen." => {
            "选择你存放音乐工程的文件夹。\nLibrarian 会找出其中所有 Ableton Live 和 FL Studio 工程，画出编曲概览，并把最新的导出准备好，随点随听。"
        }
        "Choose Folder" => "选择文件夹",

        // Empty states
        "Waiting for permission to read this folder…" => "正在等待读取此文件夹的权限…",
        "macOS may be asking whether Librarian can access this folder. Look for that dialog and click Allow." => {
            "macOS 可能正在询问是否允许 Librarian 访问这个文件夹，请找到那个对话框并点击“允许”。"
        }
        "Looking for projects…" => "正在查找工程…",
        "macOS is blocking access to this folder." => "macOS 阻止了对这个文件夹的访问。",
        "Allow Librarian in System Settings › Privacy & Security › Files and Folders, then press Refresh." => {
            "请在 系统设置 › 隐私与安全性 › 文件与文件夹 中允许 Librarian，然后点击刷新。"
        }
        "No Ableton Live or FL Studio projects found here." => "这里没有找到 Ableton Live 或 FL Studio 工程。",
        "Nothing matches your search." => "没有匹配的结果。",

        // Cards & tiles
        "Playing" => "播放中",
        "Paused" => "已暂停",
        "Open" => "打开",
        "No bounce yet. Export a demo into the project folder to preview it here." => {
            "还没有导出。把 Demo 导出到工程文件夹里，就能在这里试听。"
        }
        "No bounce yet" => "还没有导出",
        "BOUNCES" => "导出",
        "FL PROJECT FILES" => "FL 工程文件",
        "LIVE SETS" => "Live 工程文件",
        "Show in Finder" => "在访达中显示",
        "Show in Explorer" => "在资源管理器中显示",

        // Issues
        "No issues. Everything is where it should be." => "没有问题，一切都在该在的位置。",
        "No issues match your search." => "没有匹配的问题。",
        "Show again" => "重新显示",
        "Ignore" => "忽略",
        "(top of your folder)" => "（文件夹顶层）",
        "Project folder without a Live set" => "工程文件夹里没有 Live 工程文件",
        "Live set can't be read" => "工程文件无法读取",
        "Bounces outside a project" => "工程外的导出音频",
        "Live set outside a project folder" => "不在工程文件夹里的 Live 工程",
        "Bounces tucked in a subfolder" => "藏在子文件夹里的导出",
        "Set name doesn't match its folder" => "工程文件名和文件夹名不一致",
        "FL projects sharing a folder" => "多个 FL 工程共用一个文件夹",
        "The folder is marked as a Live project but has no .als at its top level." => {
            "这个文件夹被标记为 Live 工程，但顶层没有 .als 文件。"
        }
        "The file couldn't be decompressed or parsed. It may be truncated or from an unsupported version." => {
            "文件无法解压或解析，可能已损坏，或来自不支持的版本。"
        }
        "Audio that isn't next to any .als, so Librarian can't attach it to a project. Move it into its project folder." => {
            "这些音频旁边没有任何工程文件，Librarian 没法把它们归到某个工程。请把它们移到对应的工程文件夹里。"
        }
        "Live keeps samples and backups next to the set in a \"… Project\" folder. A lone .als can lose track of its files." => {
            "Live 会把采样和备份放在“… Project”文件夹里、和工程文件放在一起。单独放着的 .als 可能会找不到它用到的文件。"
        }
        "Librarian only previews bounces at the top level of a project. These are inside a subfolder." => {
            "Librarian 只预览工程文件夹顶层的导出，这些在子文件夹里。"
        }
        "There's no \"<folder name>.als\". Usually the folder was renamed after creation; Librarian uses the newest set." => {
            "没有“<文件夹名>.als”。通常是创建后改过文件夹名；Librarian 会使用最新的工程文件。"
        }
        "Each FL project should have its own folder with its bounces next to it (like hypertrance/). Here several share one folder, so bounces can only be matched by file name." => {
            "每个 FL 工程最好有自己的文件夹，导出放在旁边（比如 hypertrance/）。这里几个工程共用一个文件夹，只能按文件名匹配导出。"
        }

        // Context menu
        "Other Project Files" => "其他工程文件",
        "Other Sets" => "其他 Live 工程",
        "Play" => "播放",
        "Pause" => "暂停",
        "Space" => "空格",
        "Mark Bounces as Heard" => "标记为已听",
        "No bounces yet" => "还没有导出",
        "Tags" => "标签",
        "Copy Path" => "拷贝路径",
        "New tag…" => "新建标签…",

        // Built-in tags
        "Finished" => "已完成",
        "In Progress" => "进行中",
        "Released" => "已发布",
        "Idea" => "灵感",
        "Abandoned" => "已弃坑",
        "Other" => "其他",
        "Untagged" => "未分类",

        // Settings
        "General" => "通用",
        "Language" => "语言",
        "Appearance" => "外观",
        "System" => "跟随系统",
        "Light" => "浅色",
        "Dark" => "深色",
        "Projects folder" => "工程文件夹",
        "Change…" => "更改…",
        "DAW apps" => "DAW 应用",
        "The Open buttons launch projects with these apps." => "“打开”按钮会用这些应用打开工程。",
        "Found automatically" => "自动找到",
        "Chosen by you" => "你选择的",
        "Not found. Projects open with the system default." => "没找到，会用系统默认方式打开。",
        "Choose…" => "选择…",
        "Use automatic" => "恢复自动",
        "Playback" => "播放",
        "Output device" => "输出设备",
        "Librarian previews on this device, independent of your DAW." => "Librarian 在这个设备上试听，不影响 DAW 的设置。",
        "System default" => "系统默认",
        "Preferred format" => "优先格式",
        "When a bounce exists as both WAV and MP3, play this one." => "同一个导出同时有 WAV 和 MP3 时，优先播放哪个。",
        "Library" => "资料库",
        "Watch the folder for changes" => "监视文件夹变化",
        "New bounces and saved projects show up on their own." => "新导出和保存过的工程会自动出现。",
        "Mark unheard bounces as NEW" => "标记没听过的导出",
        "Shows a NEW chip until you play the bounce." => "在播放之前显示“新”标记。",
        "Keep tags in the projects folder" => "把标签保存在工程文件夹里",
        "Saved as .librarian-tags.txt, so tags travel with the folder (iCloud, Dropbox, an external drive)." => {
            "保存为 .librarian-tags.txt，标签会跟着文件夹走（iCloud、Dropbox、移动硬盘）。"
        }
        "Ignored issues" => "已忽略的问题",
        "None" => "无",
        "Storage" => "存储",
        "Cache" => "缓存",
        "Thumbnails and waveforms. Rebuilt automatically when needed." => "缩略图和波形，需要时会自动重建。",
        "Clear" => "清除",
        "Calculating…" => "正在计算…",
        "Settings files" => "设置文件",
        "Preferences, tags and listening history." => "偏好设置、标签和收听记录。",
        "Keyboard shortcuts" => "键盘快捷键",
        "Play / pause" => "播放 / 暂停",
        "Previous / next bounce of the playing project" => "正在播放的工程的上一个 / 下一个导出",
        "Skip back / forward 5 seconds" => "后退 / 前进 5 秒",
        "Open settings" => "打开设置",
        "Close a menu or the settings" => "关闭菜单或设置",
        "Open the project in its DAW" => "在 DAW 中打开工程",
        "Double-click" => "双击",
        "About" => "关于",
        "Made by Dyrox" => "由 Dyrox 制作",
        "Source code" => "源代码",
        "A project browser for Ableton Live and FL Studio." => "为 Ableton Live 和 FL Studio 打造的工程浏览器。",
        "Done" => "完成",
        _ => s,
    }
}

// ---- strings with numbers ---------------------------------------------------

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn n_projects(n: usize) -> String {
    if zh() { format!("{n} 个工程") } else { plural(n, "project", "projects") }
}

pub fn n_issues(n: usize) -> String {
    if zh() { format!("{n} 个问题") } else { plural(n, "issue", "issues") }
}

pub fn tidy(n: usize) -> String {
    if zh() { format!("{n} 处需要整理") } else { format!("{} to tidy up", plural(n, "thing", "things")) }
}

pub fn n_new(n: usize) -> String {
    match (zh(), n) {
        (true, 1) => "新".into(),
        (true, _) => format!("{n} 个新"),
        (false, 1) => "NEW".into(),
        (false, _) => format!("{n} NEW"),
    }
}

pub fn n_tracks(n: u32) -> String {
    if zh() { format!("{n} 轨") } else { plural(n as usize, "track", "tracks") }
}

pub fn n_channels(n: u32) -> String {
    if zh() { format!("{n} 个通道") } else { plural(n as usize, "channel", "channels") }
}

pub fn show_ignored(n: usize) -> String {
    if zh() { format!("显示 {n} 个已忽略") } else { format!("Show {n} ignored") }
}

pub fn n_ignored(n: usize) -> String {
    if zh() { format!("已忽略 {n} 个") } else { format!("{n} ignored") }
}

pub fn probably(owner: &str) -> String {
    if zh() { format!("可能属于 {owner}") } else { format!("probably {owner}") }
}

pub fn more(n: usize) -> String {
    if zh() { format!("+ 还有 {n} 个") } else { format!("+ {n} more") }
}

pub fn open_in(app: &str) -> String {
    if zh() { format!("用 {app} 打开") } else { format!("Open in {app}") }
}

pub fn bounces_n(n: usize) -> String {
    if zh() { format!("导出版本  ({n})") } else { format!("Bounces  ({n})") }
}

pub fn couldnt_play(name: &str, e: &str) -> String {
    if zh() { format!("无法播放 {name}：{e}") } else { format!("Couldn't play {name}: {e}") }
}

pub fn choose_app(app: &str) -> String {
    if zh() { format!("选择 {app} 应用") } else { format!("Choose the {app} app") }
}

/// Track/pattern counts under an expanded card.
pub fn fact(n: u32, what: &str) -> String {
    if !zh() {
        return format!("{n} {what}");
    }
    let w = match what {
        "audio" => "音频轨",
        "MIDI" => "MIDI 轨",
        "group" => "编组轨",
        "return" => "返回轨",
        "patterns" => "个 Pattern",
        "channels" => "个通道",
        other => other,
    };
    format!("{n} {w}")
}
