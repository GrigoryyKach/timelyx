//! Built-in classification of programs: which category an executable belongs to and what to call it.
//! Rules go by the executable name and path only; window titles are never used here, except to
//! tell a terminal running vim from a plain terminal.

use crate::cats;

/// File name of a path without directories or `.exe`, lower case. Both separators are understood, so
/// Windows paths work on every platform (and in tests).
pub fn stem(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let lower = name.to_lowercase();
    lower.strip_suffix(".exe").map(str::to_string).unwrap_or(lower)
}

fn lower_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

const BROWSERS: &[&str] = &[
    "chrome", "msedge", "firefox", "opera", "opera_gx", "brave", "vivaldi", "yandex", "arc", "zen", "chromium", "librewolf",
    "waterfox", "floorp", "thorium", "iexplore", "google-chrome", "google-chrome-stable", "chromium-browser", "microsoft-edge",
    "microsoft-edge-stable", "brave-browser", "firefox-bin", "firefox-esr", "vivaldi-bin", "zen-browser", "epiphany", "falkon",
    "chrome_proxy", "operagx", "tor browser", "msedge_proxy", "yandexbrowser",
];

const CODE: &[&str] = &[
    "code", "code - insiders", "cursor", "windsurf", "devenv", "sublime_text", "sublime_merge", "notepad++", "zed", "idea64", "idea",
    "pycharm64", "pycharm", "webstorm64", "webstorm", "clion64", "clion", "rider64", "rider", "goland64", "goland", "phpstorm64",
    "phpstorm", "datagrip64", "datagrip", "rubymine64", "rubymine", "rustrover64", "rustrover", "dataspell64", "studio64",
    "studio", "fleet", "appcode", "eclipse", "netbeans", "atom", "vscodium", "codium", "kate", "emacs", "gvim", "nvim-qt",
    "godot", "unity", "unrealeditor", "githubdesktop", "gitkraken", "sourcetree", "postman", "dbeaver", "docker desktop",
    "insomnia", "heidisql", "ssms", "azuredatastudio", "arduino ide", "arduino-ide", "gedit", "gnome-builder", "geany",
];

const COMMUNICATION: &[&str] = &[
    "telegram", "discord", "slack", "teams", "ms-teams", "msteams", "zoom", "whatsapp", "signal", "skype", "viber", "element",
    "mattermost", "thunderbird", "outlook", "olk", "webex", "ciscocollabhost", "telegram-desktop", "discordptb", "discordcanary",
    "vesktop", "legcord", "evolution", "geary", "caprine",
];

const VIDEO: &[&str] = &[
    "vlc", "mpv", "mpc-hc", "mpc-hc64", "mpc-be", "mpc-be64", "potplayer", "potplayermini", "potplayermini64", "wmplayer",
    "microsoft.media.player", "spotify", "itunes", "foobar2000", "yandexmusic", "obs64", "obs32", "obs", "obs-studio", "kodi",
    "plex", "plexamp", "tidal", "deezer", "musicbee", "audacious", "rhythmbox", "celluloid", "totem", "smplayer", "streamlabs obs",
];

const OFFICE: &[&str] = &[
    "winword", "excel", "powerpnt", "onenote", "msaccess", "mspub", "soffice", "soffice.bin", "libreoffice", "obsidian", "notion",
    "evernote", "acrobat", "acrord32", "acrord64", "foxitreader", "foxitpdfreader", "sumatrapdf", "okular", "evince", "typora",
    "logseq", "wps", "wpsoffice", "et", "wpp", "onlyoffice", "desktopeditors", "calibre", "ebook-viewer", "todoist", "ticktick",
    "notepad", "joplin", "zotero", "xournalpp", "libreoffice-writer", "lowriter", "localc", "loimpress", "masterpdfeditor",
];

const DESIGN: &[&str] = &[
    "figma", "photoshop", "illustrator", "indesign", "afterfx", "adobe premiere pro", "premiere", "blender", "gimp", "gimp-2.10",
    "gimp-3.0", "krita", "inkscape", "affinity photo", "affinity designer", "affinity publisher", "resolve", "mspaint",
    "paintdotnet", "clipstudiopaint", "lightroom", "xd", "cinema 4d", "3dsmax", "maya", "zbrush", "aseprite", "fusion360",
    "audacity", "ableton live", "fl64", "fl", "reaper", "sketchup", "scribus", "darktable", "rawtherapee", "kdenlive", "shotcut",
    "openshot", "olive", "handbrake", "canva", "lunacy", "penpot",
];

const SYSTEM: &[&str] = &[
    "explorer", "windowsterminal", "wt", "cmd", "powershell", "powershell_ise", "pwsh", "conhost", "openconsole", "alacritty",
    "wezterm-gui", "wezterm", "kitty", "mintty", "terminal", "gnome-terminal", "gnome-terminal-server", "konsole", "xterm",
    "tilix", "foot", "terminator", "ptyxis", "systemsettings", "taskmgr", "regedit", "control", "mmc", "applicationframehost",
    "searchhost", "searchapp", "startmenuexperiencehost", "shellexperiencehost", "lockapp", "logonui", "nautilus", "dolphin",
    "thunar", "nemo", "pcmanfm", "caja", "gnome-control-center", "gnome-system-monitor", "msiexec", "snippingtool",
    "screenclippingtool", "calc", "perfmon", "resmon", "eventvwr", "compmgmt", "diskmgmt", "services", "winver", "dwm",
    "textinputhost", "searchui", "sihost", "systeminformer", "processhacker", "procexp", "procexp64", "winrar", "7zfm",
    "peazip", "ark", "file-roller", "gnome-calculator",
];

const GAMES: &[&str] = &[
    "steam", "steamwebhelper", "epicgameslauncher", "galaxyclient", "battle.net", "battlenet", "ubisoftconnect", "upc",
    "eadesktop", "origin", "riotclientservices", "riotclientux", "leagueclientux", "leagueclient", "valorant", "valorant-win64-shipping",
    "minecraftlauncher", "minecraft", "minecraft-launcher", "prismlauncher", "robloxplayerbeta", "robloxplayer", "genshinimpact",
    "yuanshen", "dota2", "cs2", "csgo", "hl2", "gta5", "fortniteclient-win64-shipping", "wow", "wowclassic", "overwatch",
    "r5apex", "eldenring", "cyberpunk2077", "witcher3", "starfield", "lutris", "heroic", "bottles", "retroarch", "pcsx2-qt",
    "rpcs3", "yuzu", "ryujinx", "dolphin-emu", "citra-qt", "xemu", "playnite.desktopapp", "playnite.fullscreenapp", "goggalaxy",
    "gog galaxy", "xboxapp", "gamingapp", "wotblitz", "worldoftanks", "warthunder", "aces",
];

const TERMINALS: &[&str] = &[
    "windowsterminal", "wt", "cmd", "powershell", "pwsh", "conhost", "openconsole", "alacritty", "wezterm-gui", "wezterm", "kitty",
    "mintty", "terminal", "gnome-terminal", "gnome-terminal-server", "konsole", "xterm", "tilix", "foot", "terminator", "ptyxis",
];

pub fn is_browser(path: &str) -> bool {
    let s = stem(path);
    if s == "browser" {
        // Yandex Browser ships as a bare `browser.exe`.
        return lower_path(path).contains("yandex");
    }
    if s == "firefox" || s == "chrome" {
        return true;
    }
    BROWSERS.contains(&s.as_str())
}

pub fn is_terminal(path: &str) -> bool {
    TERMINALS.contains(&stem(path).as_str())
}

/// Is the path inside the library folder of a game store or launcher?
pub fn is_game_path(path: &str) -> bool {
    let p = lower_path(path);
    const MARKS: &[&str] = &[
        "/steamapps/common/",
        "/steamapps/",
        "/epic games/",
        "/epicgames/",
        "/gog galaxy/games/",
        "/gog games/",
        "/goggames/",
        "/battle.net/",
        "/battle.net games/",
        "/blizzard/",
        "/ea games/",
        "/electronic arts/",
        "/origin games/",
        "/ubisoft game launcher/games/",
        "/ubisoft/ubisoft game launcher/games/",
        "/riot games/",
        "/xboxgames/",
        "/.steam/",
        "/steam/steamapps/",
        "/lutris/",
        "/games/heroic/",
        "/heroic/",
    ];
    MARKS.iter().any(|m| p.contains(m))
}

/// The built-in category of an executable.
pub fn classify(path: &str) -> &'static str {
    if is_browser(path) {
        return cats::BROWSER;
    }
    let s = stem(path);
    let s = s.as_str();
    // A game library folder beats the name: a tool called `code.exe` inside `steamapps\common` is part of a game.
    if is_game_path(path) {
        return cats::GAMES;
    }
    for (list, cat) in [
        (CODE, cats::CODE),
        (COMMUNICATION, cats::COMMUNICATION),
        (VIDEO, cats::VIDEO),
        (OFFICE, cats::OFFICE),
        (DESIGN, cats::DESIGN),
        (GAMES, cats::GAMES),
        (SYSTEM, cats::SYSTEM),
    ] {
        if list.contains(&s) {
            return cat;
        }
    }
    if s.starts_with("jetbrains") || s.starts_with("libreoffice") {
        return if s.starts_with("jetbrains") { cats::CODE } else { cats::OFFICE };
    }
    cats::OTHER
}

/// Does a terminal title look like an editor session (nvim, vim, helix)? Only used to pick a category;
/// the title itself is never stored.
pub fn title_looks_like_vim(title: &str) -> bool {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| matches!(w, "nvim" | "vim" | "neovim" | "helix"))
}

const NAMES: &[(&str, &str)] = &[
    ("chrome", "Google Chrome"),
    ("google-chrome", "Google Chrome"),
    ("google-chrome-stable", "Google Chrome"),
    ("msedge", "Microsoft Edge"),
    ("microsoft-edge", "Microsoft Edge"),
    ("microsoft-edge-stable", "Microsoft Edge"),
    ("firefox", "Firefox"),
    ("firefox-bin", "Firefox"),
    ("opera", "Opera"),
    ("opera_gx", "Opera GX"),
    ("brave", "Brave"),
    ("brave-browser", "Brave"),
    ("vivaldi", "Vivaldi"),
    ("vivaldi-bin", "Vivaldi"),
    ("yandex", "Yandex Browser"),
    ("browser", "Yandex Browser"),
    ("zen", "Zen Browser"),
    ("arc", "Arc"),
    ("code", "Visual Studio Code"),
    ("code - insiders", "VS Code Insiders"),
    ("cursor", "Cursor"),
    ("devenv", "Visual Studio"),
    ("sublime_text", "Sublime Text"),
    ("notepad++", "Notepad++"),
    ("idea64", "IntelliJ IDEA"),
    ("idea", "IntelliJ IDEA"),
    ("pycharm64", "PyCharm"),
    ("pycharm", "PyCharm"),
    ("webstorm64", "WebStorm"),
    ("clion64", "CLion"),
    ("rider64", "Rider"),
    ("goland64", "GoLand"),
    ("phpstorm64", "PhpStorm"),
    ("datagrip64", "DataGrip"),
    ("rubymine64", "RubyMine"),
    ("rustrover64", "RustRover"),
    ("studio64", "Android Studio"),
    ("explorer", "File Explorer"),
    ("windowsterminal", "Windows Terminal"),
    ("wt", "Windows Terminal"),
    ("cmd", "Command Prompt"),
    ("powershell", "PowerShell"),
    ("pwsh", "PowerShell"),
    ("conhost", "Console"),
    ("applicationframehost", "Windows app"),
    ("systemsettings", "Settings"),
    ("taskmgr", "Task Manager"),
    ("winword", "Word"),
    ("excel", "Excel"),
    ("powerpnt", "PowerPoint"),
    ("onenote", "OneNote"),
    ("outlook", "Outlook"),
    ("olk", "Outlook"),
    ("teams", "Microsoft Teams"),
    ("ms-teams", "Microsoft Teams"),
    ("msteams", "Microsoft Teams"),
    ("telegram", "Telegram"),
    ("telegram-desktop", "Telegram"),
    ("discord", "Discord"),
    ("slack", "Slack"),
    ("zoom", "Zoom"),
    ("whatsapp", "WhatsApp"),
    ("spotify", "Spotify"),
    ("vlc", "VLC"),
    ("mpv", "mpv"),
    ("obs64", "OBS Studio"),
    ("obs", "OBS Studio"),
    ("photoshop", "Photoshop"),
    ("illustrator", "Illustrator"),
    ("figma", "Figma"),
    ("blender", "Blender"),
    ("steam", "Steam"),
    ("steamwebhelper", "Steam"),
    ("epicgameslauncher", "Epic Games Launcher"),
    ("soffice", "LibreOffice"),
    ("soffice.bin", "LibreOffice"),
    ("mspaint", "Paint"),
    ("notepad", "Notepad"),
    ("gnome-terminal-server", "GNOME Terminal"),
    ("gnome-terminal", "GNOME Terminal"),
    ("konsole", "Konsole"),
    ("nautilus", "Files"),
];

/// A readable name for an executable: the known name, otherwise the file name with a capital letter.
pub fn display_name(path: &str) -> String {
    let s = stem(path);
    if s == "browser" && !lower_path(path).contains("yandex") {
        return "Browser".into();
    }
    if let Some((_, n)) = NAMES.iter().find(|(k, _)| *k == s) {
        return (*n).to_string();
    }
    let cleaned: String = s.chars().map(|c| if c == '_' || c == '-' || c == '.' { ' ' } else { c }).collect();
    let cleaned = cleaned.trim();
    let mut it = cleaned.chars();
    match it.next() {
        Some(f) => f.to_uppercase().collect::<String>() + it.as_str(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_handles_both_separators_and_exe() {
        assert_eq!(stem(r"C:\Program Files\Google\Chrome\Application\chrome.EXE"), "chrome");
        assert_eq!(stem("/usr/bin/firefox"), "firefox");
        assert_eq!(stem("Code - Insiders.exe"), "code - insiders");
    }

    #[test]
    fn browsers_are_recognised() {
        for p in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Mozilla Firefox\firefox.exe",
            r"C:\Users\a\AppData\Local\Programs\Opera\opera.exe",
            r"C:\Users\a\AppData\Local\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"C:\Users\a\AppData\Local\Vivaldi\Application\vivaldi.exe",
            r"C:\Users\a\AppData\Local\Yandex\YandexBrowser\Application\browser.exe",
            "/usr/bin/google-chrome-stable",
            "/opt/zen/zen",
        ] {
            assert!(is_browser(p), "{p}");
            assert_eq!(classify(p), cats::BROWSER, "{p}");
        }
        assert!(!is_browser(r"C:\Tools\browser.exe"));
    }

    #[test]
    fn code_editors_and_ides() {
        for p in [
            r"C:\Users\a\AppData\Local\Programs\Microsoft VS Code\Code.exe",
            r"C:\Users\a\AppData\Local\Programs\cursor\Cursor.exe",
            r"C:\Program Files\JetBrains\IntelliJ IDEA 2026.1\bin\idea64.exe",
            r"C:\Program Files\JetBrains\PyCharm\bin\pycharm64.exe",
            r"C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\devenv.exe",
            r"C:\Program Files\Android\Android Studio\bin\studio64.exe",
            r"C:\Program Files\Sublime Text\sublime_text.exe",
            r"C:\Program Files\Notepad++\notepad++.exe",
        ] {
            assert_eq!(classify(p), cats::CODE, "{p}");
        }
    }

    #[test]
    fn other_built_in_categories() {
        assert_eq!(classify(r"C:\Users\a\AppData\Roaming\Telegram Desktop\Telegram.exe"), cats::COMMUNICATION);
        assert_eq!(classify(r"C:\Users\a\AppData\Local\Discord\app-1.0.9\Discord.exe"), cats::COMMUNICATION);
        assert_eq!(classify(r"C:\Program Files\VideoLAN\VLC\vlc.exe"), cats::VIDEO);
        assert_eq!(classify(r"C:\Users\a\AppData\Roaming\Spotify\Spotify.exe"), cats::VIDEO);
        assert_eq!(classify(r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE"), cats::OFFICE);
        assert_eq!(classify(r"C:\Users\a\AppData\Local\Obsidian\Obsidian.exe"), cats::OFFICE);
        assert_eq!(classify(r"C:\Program Files\Adobe\Adobe Photoshop 2026\Photoshop.exe"), cats::DESIGN);
        assert_eq!(classify(r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe"), cats::DESIGN);
        assert_eq!(classify(r"C:\Windows\explorer.exe"), cats::SYSTEM);
        assert_eq!(classify(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\WindowsTerminal.exe"), cats::SYSTEM);
        assert_eq!(classify(r"C:\Tools\mystery.exe"), cats::OTHER);
    }

    #[test]
    fn steam_and_other_library_paths_are_games() {
        for p in [
            r"C:\Program Files (x86)\Steam\steamapps\common\Hades\Hades.exe",
            r"D:\SteamLibrary\steamapps\common\Some Game\game.exe",
            r"C:\Program Files\Epic Games\Fortnite\FortniteGame\Binaries\Win64\FortniteClient-Win64-Shipping.exe",
            r"C:\GOG Games\Witcher\witcher.exe",
            r"C:\Program Files (x86)\Battle.net\Battle.net.exe",
            r"C:\Program Files\EA Games\Apex\r5apex.exe",
            r"C:\Program Files\Ubisoft\Ubisoft Game Launcher\games\Far Cry 6\FC6.exe",
            r"C:\Riot Games\VALORANT\live\VALORANT.exe",
            "/home/u/.local/share/Steam/steamapps/common/Hades/hades",
        ] {
            assert!(is_game_path(p), "{p}");
            assert_eq!(classify(p), cats::GAMES, "{p}");
        }
        assert!(!is_game_path(r"C:\Program Files\Mozilla Firefox\firefox.exe"));
        // The Steam client itself sits outside steamapps but is still a games program.
        assert_eq!(classify(r"C:\Program Files (x86)\Steam\steam.exe"), cats::GAMES);
    }

    #[test]
    fn terminals_and_vim_titles() {
        assert!(is_terminal(r"C:\Program Files\WindowsApps\x\WindowsTerminal.exe"));
        assert!(!is_terminal(r"C:\Windows\explorer.exe"));
        assert!(title_looks_like_vim("nvim main.rs"));
        assert!(title_looks_like_vim("main.rs - NVIM"));
        assert!(title_looks_like_vim("vim ~/notes.txt"));
        assert!(!title_looks_like_vim("cargo build"));
        assert!(!title_looks_like_vim("C:\\Windows\\system32\\cmd.exe"));
    }

    #[test]
    fn names_are_readable() {
        assert_eq!(display_name(r"C:\x\Code.exe"), "Visual Studio Code");
        assert_eq!(display_name(r"C:\x\msedge.exe"), "Microsoft Edge");
        assert_eq!(display_name(r"C:\x\some_tool-name.exe"), "Some tool name");
        assert_eq!(display_name(r"C:\Users\a\AppData\Local\Yandex\Application\browser.exe"), "Yandex Browser");
    }
}
