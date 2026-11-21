//! Browser tabs: turns a window title into a site name.
//!
//! Browsers put the page title and their own name in the window title, "Page - Site - Google Chrome". We keep only
//! the normalised **site name** (the brand where it is clear, otherwise the site part of the title) and never the page
//! title. Private windows are recognised by their marker and recorded as [`PRIVATE`] without a site.

/// Stored in place of a site name for private windows. Shown as "Private browsing".
pub const PRIVATE: &str = "*private";
/// Stored when the title says nothing about the site (new tab, a bare page title). Shown as "Other sites".
pub const OTHER: &str = "*other";

/// Is `name` one of the two special values rather than a real site?
pub fn is_special(name: &str) -> bool {
    name.starts_with('*')
}

const INVISIBLE: &[char] = &['\u{200b}', '\u{200c}', '\u{200d}', '\u{200e}', '\u{200f}', '\u{feff}', '\u{2060}'];

const PRIVATE_MARKERS: &[&str] = &[
    "incognito",
    "inprivate",
    "private browsing",
    "private window",
    "(private)",
    "инкогнито",
    "приватн",
    "частный просмотр",
    "анонимн",
    "анонімн",
    "приватне",
];

const BROWSER_NAMES: &[&str] = &[
    "google chrome",
    "chrome",
    "chromium",
    "microsoft edge",
    "microsoftedge",
    "edge",
    "mozilla firefox",
    "firefox",
    "firefox developer edition",
    "opera",
    "opera gx",
    "brave",
    "vivaldi",
    "yandex browser",
    "яндекс браузер",
    "яндекс.браузер",
    "яндекс браузер",
    "zen browser",
    "zen",
    "arc",
    "tor browser",
    "librewolf",
    "waterfox",
    "floorp",
    "thorium",
    "safari",
    "яндекс",
];

const SEPARATORS: &[&str] = &[" - ", " — ", " – ", " | ", " · ", " • ", " :: ", " / "];

fn normalise_spaces(s: &str) -> String {
    s.chars().filter(|c| !INVISIBLE.contains(c)).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Does the window title say that the window is private?
pub fn is_private(title: &str) -> bool {
    let l = normalise_spaces(title).to_lowercase();
    PRIVATE_MARKERS.iter().any(|m| l.contains(m))
}

/// Cuts "and 3 more pages" (and the Russian and Ukrainian forms) from the end of a Chrome-style title.
fn strip_more_pages(s: &str) -> String {
    let words: Vec<&str> = s.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        if let Some(len) = more_pages_at(&words[i..]) {
            i += len;
        } else {
            out.push(words[i]);
            i += 1;
        }
    }
    out.join(" ")
}

/// Length in words of an "and 3 more pages" phrase that starts at `w[0]`.
fn more_pages_at(w: &[&str]) -> Option<usize> {
    let lower = |x: &str| x.to_lowercase();
    if !matches!(lower(w.first()?).as_str(), "and" | "и" | "та") {
        return None;
    }
    let mut k = 1;
    if w.get(k).is_some_and(|x| matches!(lower(x).as_str(), "ещё" | "еще" | "ще")) {
        k += 1;
    }
    let n = w.get(k)?;
    if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    k += 1;
    let mut noun = false;
    for _ in 0..2 {
        match w.get(k).map(|x| lower(x)) {
            Some(x) if matches!(x.as_str(), "more" | "page" | "pages" | "tab" | "tabs" | "страница" | "страницы" | "страниц" | "вкладка" | "вкладки" | "вкладок" | "сторінка" | "сторінки" | "сторінок" | "вкладку") => {
                k += 1;
                noun = true;
            }
            _ => break,
        }
    }
    noun.then_some(k)
}

/// Removes the trailing browser name(s) ("... - Google Chrome", "... — Mozilla Firefox") and the "N more pages" note.
pub fn strip_browser(title: &str) -> String {
    let mut t = normalise_spaces(title);
    loop {
        let mut cut = None;
        for sep in SEPARATORS.iter().take(3) {
            if let Some(i) = t.rfind(sep) {
                let tail = t[i + sep.len()..].trim().to_lowercase();
                let tail = tail.trim_start_matches(|c: char| c == '[' || c == '(').trim_end_matches(|c: char| c == ']' || c == ')');
                if BROWSER_NAMES.contains(&tail) {
                    cut = Some(i);
                    break;
                }
            }
        }
        match cut {
            Some(i) => t = t[..i].trim_end().to_string(),
            None => break,
        }
    }
    // A bare browser name as the whole title (an empty new tab).
    if BROWSER_NAMES.contains(&t.to_lowercase().as_str()) {
        return String::new();
    }
    strip_more_pages(&t)
}

struct Brand {
    name: &'static str,
    /// Lower-case segment values that identify the site (exact match of one title segment).
    aliases: &'static [&'static str],
}

const BRANDS: &[Brand] = &[
    Brand { name: "YouTube", aliases: &["youtube", "youtube music", "ютуб", "youtube studio"] },
    Brand { name: "GitHub", aliases: &["github", "гитхаб", "гітхаб"] },
    Brand { name: "GitLab", aliases: &["gitlab"] },
    Brand { name: "Gmail", aliases: &["gmail"] },
    Brand { name: "Stack Overflow", aliases: &["stack overflow", "stackoverflow", "stack exchange"] },
    Brand { name: "Reddit", aliases: &["reddit", "reddit - dive into anything"] },
    Brand { name: "Twitch", aliases: &["twitch"] },
    Brand { name: "ChatGPT", aliases: &["chatgpt", "openai"] },
    Brand { name: "Claude", aliases: &["claude", "claude.ai"] },
    Brand { name: "Google", aliases: &["google", "google search", "поиск в google", "пошук google", "пошук у google"] },
    Brand { name: "Google Docs", aliases: &["google docs", "google документы", "google документи"] },
    Brand { name: "Google Sheets", aliases: &["google sheets", "google таблицы", "google таблиці"] },
    Brand { name: "Google Slides", aliases: &["google slides", "google презентации", "google презентації"] },
    Brand { name: "Google Drive", aliases: &["google drive", "google диск"] },
    Brand { name: "Google Calendar", aliases: &["google calendar", "google календарь", "google календар"] },
    Brand { name: "Google Maps", aliases: &["google maps", "google карты", "google карти"] },
    Brand { name: "Google Meet", aliases: &["google meet", "meet"] },
    Brand { name: "Wikipedia", aliases: &["wikipedia", "википедия", "вікіпедія"] },
    Brand { name: "X", aliases: &["x", "twitter", "x (formerly twitter)"] },
    Brand { name: "Facebook", aliases: &["facebook", "фейсбук"] },
    Brand { name: "Instagram", aliases: &["instagram", "инстаграм", "інстаграм"] },
    Brand { name: "LinkedIn", aliases: &["linkedin"] },
    Brand { name: "Netflix", aliases: &["netflix"] },
    Brand { name: "Telegram", aliases: &["telegram", "telegram web", "web telegram"] },
    Brand { name: "WhatsApp", aliases: &["whatsapp", "whatsapp web"] },
    Brand { name: "Discord", aliases: &["discord"] },
    Brand { name: "Slack", aliases: &["slack"] },
    Brand { name: "Notion", aliases: &["notion"] },
    Brand { name: "Figma", aliases: &["figma"] },
    Brand { name: "Outlook", aliases: &["outlook", "outlook.com", "mail - outlook"] },
    Brand { name: "Microsoft Teams", aliases: &["microsoft teams", "teams"] },
    Brand { name: "Yandex", aliases: &["yandex", "яндекс", "яндекс почта", "яндекс.почта", "яндекс музыка", "яндекс карты"] },
    Brand { name: "VK", aliases: &["vk", "вконтакте", "вк", "vk.com"] },
    Brand { name: "Habr", aliases: &["habr", "хабр", "habr.com"] },
    Brand { name: "Pinterest", aliases: &["pinterest"] },
    Brand { name: "Amazon", aliases: &["amazon", "amazon.com"] },
    Brand { name: "eBay", aliases: &["ebay"] },
    Brand { name: "Spotify", aliases: &["spotify", "spotify – web player", "spotify - web player"] },
    Brand { name: "Steam", aliases: &["steam", "steam community", "сообщество steam", "спільнота steam"] },
    Brand { name: "Medium", aliases: &["medium"] },
    Brand { name: "TikTok", aliases: &["tiktok"] },
    Brand { name: "Bing", aliases: &["bing", "microsoft bing"] },
    Brand { name: "DuckDuckGo", aliases: &["duckduckgo", "duck duck go"] },
    Brand { name: "Mail.ru", aliases: &["mail.ru", "почта mail.ru", "пошта mail.ru"] },
    Brand { name: "Rutube", aliases: &["rutube"] },
    Brand { name: "Kinopoisk", aliases: &["кинопоиск", "kinopoisk", "кіно пошук"] },
    Brand { name: "Rozetka", aliases: &["rozetka", "розетка"] },
    Brand { name: "Ukrainska Pravda", aliases: &["українська правда", "украинская правда"] },
    Brand { name: "Trello", aliases: &["trello"] },
    Brand { name: "Jira", aliases: &["jira"] },
    Brand { name: "Confluence", aliases: &["confluence"] },
    Brand { name: "Canva", aliases: &["canva"] },
    Brand { name: "Udemy", aliases: &["udemy"] },
    Brand { name: "Coursera", aliases: &["coursera"] },
    Brand { name: "MDN", aliases: &["mdn web docs", "mdn"] },
    Brand { name: "crates.io", aliases: &["crates.io", "docs.rs"] },
];

/// Strips a leading counter "(3) " or a media mark, which tabs add in front of a segment.
fn clean_segment(seg: &str) -> String {
    let mut s = seg.trim();
    loop {
        let before = s;
        if let Some(rest) = s.strip_prefix('(') {
            if let Some(close) = rest.find(')') {
                if rest[..close].chars().all(|c| c.is_ascii_digit() || c == '+') && close > 0 {
                    s = rest[close + 1..].trim_start();
                }
            }
        }
        for mark in ["▶", "●", "🔊", "🔇", "•", "*"] {
            if let Some(r) = s.strip_prefix(mark) {
                s = r.trim_start();
            }
        }
        if s == before {
            break;
        }
    }
    s.trim().to_string()
}

/// Splits a title into segments by the usual separators.
fn segments(title: &str) -> Vec<String> {
    let mut parts = vec![title.to_string()];
    for sep in SEPARATORS {
        parts = parts.into_iter().flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>()).collect();
    }
    parts.into_iter().map(|p| clean_segment(&p)).filter(|p| !p.is_empty()).collect()
}

fn brand_of(segment: &str) -> Option<&'static str> {
    let l = segment.to_lowercase();
    BRANDS.iter().find(|b| b.aliases.contains(&l.as_str())).map(|b| b.name)
}

const GENERIC: &[&str] = &[
    "new tab", "новая вкладка", "нова вкладка", "untitled", "без названия", "без назви", "about:blank", "start page",
    "speed dial", "home", "главная", "головна", "loading", "загрузка", "завантаження", "404", "error", "ошибка", "помилка",
];

/// A site name taken from the last segment of an unknown title, or `None` when it looks like a page title.
fn plausible_site(seg: &str) -> Option<String> {
    let s = seg.trim();
    let n = s.chars().count();
    if !(2..=28).contains(&n) || s.contains('@') || s.contains("://") || s.chars().all(|c| !c.is_alphabetic()) {
        return None;
    }
    if GENERIC.contains(&s.to_lowercase().as_str()) {
        return None;
    }
    // A domain looks like one word with a dot.
    if s.contains('.') && !s.contains(' ') {
        let d = s.trim_start_matches("www.").trim_end_matches('.').to_lowercase();
        if d.split('.').count() >= 2 && d.split('.').all(|p| !p.is_empty()) {
            return Some(d);
        }
    }
    // Too many words: that is a sentence, not a name.
    if s.split_whitespace().count() > 3 {
        return None;
    }
    Some(s.to_string())
}

/// The site for a browser window title: a brand, the site part of the title, [`PRIVATE`] or [`OTHER`].
pub fn parse(title: &str) -> String {
    if is_private(title) {
        return PRIVATE.to_string();
    }
    let core = strip_browser(title);
    if core.trim().is_empty() {
        return OTHER.to_string();
    }
    let segs = segments(&core);
    // Reddit post titles end with ": r/subreddit" and carry no brand.
    if segs.iter().any(|s| s.to_lowercase().starts_with("r/")) || core.to_lowercase().contains(" : r/") {
        return "Reddit".to_string();
    }
    for seg in segs.iter().rev() {
        if let Some(b) = brand_of(seg) {
            return b.to_string();
        }
    }
    // A brand in a middle segment ("Inbox - GitHub - Notifications") still counts.
    for seg in segs.iter() {
        if let Some(b) = brand_of(seg) {
            return b.to_string();
        }
    }
    if segs.len() >= 2 {
        if let Some(s) = segs.last().and_then(|l| plausible_site(l)) {
            return s;
        }
    }
    OTHER.to_string()
}

/// Is the site excluded by the user's list (case-insensitive, a part of the name matches)?
pub fn is_excluded(site: &str, exclusions: &[String]) -> bool {
    let l = site.to_lowercase();
    exclusions.iter().map(|e| e.trim().to_lowercase()).any(|e| !e.is_empty() && (l == e || l.contains(&e)))
}

/// Shipped category for well-known sites; `None` keeps the browser's category.
pub fn default_category(site: &str) -> Option<&'static str> {
    use crate::cats::*;
    Some(match site {
        "YouTube" | "Twitch" | "Netflix" | "Kinopoisk" | "Rutube" | "TikTok" | "Spotify" => VIDEO,
        "GitHub" | "GitLab" | "Stack Overflow" | "MDN" | "crates.io" | "Habr" => CODE,
        "Gmail" | "Outlook" | "Mail.ru" | "Telegram" | "WhatsApp" | "Discord" | "Slack" | "Microsoft Teams" | "Google Meet"
        | "Facebook" | "Instagram" | "VK" | "X" | "LinkedIn" => COMMUNICATION,
        "Notion" | "Google Docs" | "Google Sheets" | "Google Slides" | "Google Drive" | "Google Calendar" | "Trello" | "Jira"
        | "Confluence" => OFFICE,
        "Figma" | "Canva" => DESIGN,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(t: &str) -> String {
        parse(t)
    }

    #[test]
    fn chrome_titles() {
        assert_eq!(site("Rust Programming Language - YouTube - Google Chrome"), "YouTube");
        assert_eq!(site("owner/repo: A tiny tool · GitHub - Google Chrome"), "GitHub");
        assert_eq!(site("(3) Inbox (12) - me@example.com - Gmail - Google Chrome"), "Gmail");
        assert_eq!(site("How do I exit Vim? - Stack Overflow - Google Chrome"), "Stack Overflow");
        assert_eq!(site("Tab 1 and 3 more pages - Google Chrome"), OTHER);
        assert_eq!(site("Lo-fi beats - YouTube and 2 more pages - Google Chrome"), "YouTube");
        assert_eq!(site("New Tab - Google Chrome"), OTHER);
        assert_eq!(site("Google Chrome"), OTHER);
    }

    #[test]
    fn edge_titles_with_profile_and_zero_width_space() {
        assert_eq!(site("Pull request #12 · org/repo · GitHub - Personal - Microsoft\u{200b} Edge"), "GitHub");
        assert_eq!(site("Dashboard - Microsoft\u{200b} Edge"), OTHER);
        assert_eq!(site("Quarterly plan - Notion and 4 more pages - Work - Microsoft\u{200b} Edge"), "Notion");
    }

    #[test]
    fn firefox_titles() {
        assert_eq!(site("Why is the sky blue? — Wikipedia — Mozilla Firefox"), "Wikipedia");
        assert_eq!(site("lofi hip hop radio - YouTube — Mozilla Firefox"), "YouTube");
        assert_eq!(site("Mozilla Firefox"), OTHER);
    }

    #[test]
    fn opera_brave_vivaldi_yandex_zen_arc() {
        assert_eq!(site("streamer is live - Twitch - Opera"), "Twitch");
        assert_eq!(site("Chat - ChatGPT - Brave"), "ChatGPT");
        assert_eq!(site("Release notes - Claude - Vivaldi"), "Claude");
        assert_eq!(site("Погода в Киеве - Яндекс Браузер"), OTHER);
        assert_eq!(site("Новости - Habr - Yandex Browser"), "Habr");
        assert_eq!(site("Some video - YouTube — Zen Browser"), "YouTube");
        assert_eq!(site("Reddit - Dive into anything"), "Reddit");
        // Arc has no suffix at all.
        assert_eq!(site("Lyrics - Genius"), "Genius");
    }

    #[test]
    fn russian_and_ukrainian_titles() {
        assert_eq!(site("Как установить Rust - YouTube - Google Chrome"), "YouTube");
        assert_eq!(site("Входящие - ivan@gmail.com - Gmail и ещё 2 страницы - Google Chrome"), "Gmail");
        assert_eq!(site("Вікіпедія — вільна енциклопедія — Вікіпедія — Mozilla Firefox"), "Wikipedia");
        assert_eq!(site("Фільми онлайн - Rozetka та ще 3 сторінки - Google Chrome"), "Rozetka");
        assert_eq!(site("Фильмы онлайн - КиноПоиск - Яндекс Браузер"), "Kinopoisk");
        assert_eq!(site("Погода - Поиск в Google - Google Chrome"), "Google");
        assert_eq!(site("Новая вкладка - Google Chrome"), OTHER);
        assert_eq!(site("Нова вкладка - Google Chrome"), OTHER);
    }

    #[test]
    fn x_and_reddit_special_cases() {
        assert_eq!(site("(1) Home / X - Google Chrome"), "X");
        assert_eq!(site("Why does my borrow checker hate me : r/rust - Google Chrome"), "Reddit");
    }

    #[test]
    fn unknown_sites_keep_the_site_part_only() {
        assert_eq!(site("Release 2.0 announced - The Verge - Google Chrome"), "The Verge");
        assert_eq!(site("Docs - example.com - Google Chrome"), "example.com");
        // A single segment is a page title: never stored.
        assert_eq!(site("Weather in Kyiv tomorrow - Google Chrome"), OTHER);
        // An e-mail address or a long sentence is not a site name.
        assert_eq!(site("Account - me@example.com - Google Chrome"), OTHER);
        assert_eq!(site("Report - this is a very long sentence about my private plans - Google Chrome"), OTHER);
    }

    #[test]
    fn private_windows_have_no_site() {
        assert_eq!(site("YouTube - Google Chrome (Incognito)"), PRIVATE);
        assert_eq!(site("Секретная вкладка - Google Chrome (Инкогнито)"), PRIVATE);
        assert_eq!(site("GitHub - [InPrivate] - Microsoft\u{200b} Edge"), PRIVATE);
        assert_eq!(site("Gmail — Mozilla Firefox Private Browsing"), PRIVATE);
        assert_eq!(site("Gmail - Mozilla Firefox (Private Browsing)"), PRIVATE);
        assert_eq!(site("Приватне вікно - Mozilla Firefox"), PRIVATE);
        assert!(is_special(PRIVATE) && is_special(OTHER) && !is_special("GitHub"));
    }

    #[test]
    fn page_titles_never_leak_into_the_site() {
        for t in [
            "My secret diary entry - Google Chrome",
            "Salary negotiation tips - Google Chrome",
            "(5) Хто мій лікар? - Google Chrome",
        ] {
            let s = site(t);
            assert_eq!(s, OTHER, "{t}");
        }
    }

    #[test]
    fn exclusions_match_case_insensitively() {
        let ex = vec!["youtube".to_string(), "  ".to_string()];
        assert!(is_excluded("YouTube", &ex));
        assert!(is_excluded("YouTube Music", &ex));
        assert!(!is_excluded("GitHub", &ex));
        assert!(!is_excluded("GitHub", &[]));
    }

    #[test]
    fn shipped_site_categories() {
        use crate::cats::*;
        assert_eq!(default_category("YouTube"), Some(VIDEO));
        assert_eq!(default_category("Twitch"), Some(VIDEO));
        assert_eq!(default_category("GitHub"), Some(CODE));
        assert_eq!(default_category("Stack Overflow"), Some(CODE));
        assert_eq!(default_category("Gmail"), Some(COMMUNICATION));
        assert_eq!(default_category("Telegram"), Some(COMMUNICATION));
        assert_eq!(default_category("The Verge"), None);
    }
}
